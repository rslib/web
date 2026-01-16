//! Async/await helpers for Lua API (coroutine-based)
//!
//! Functions: async.task, async.await, async.yield, async.all, async.race, async.sleep

use mlua::{Lua, Result, Table};

/// Create the async module table
pub fn create_module(lua: &Lua) -> Result<Table> {
    // Create async module using Lua code
    let async_code = r#"
        local async = {}

        -- Create a task from a function (wraps in coroutine)
        function async.task(fn)
            return {
                _co = coroutine.create(fn),
                _completed = false,
                _result = nil,
            }
        end

        -- Run a task to completion
        function async.await(task)
            if task._completed then
                return task._result
            end
            while coroutine.status(task._co) ~= "dead" do
                local ok, result = coroutine.resume(task._co)
                if not ok then
                    error(result)
                end
                task._result = result
            end
            task._completed = true
            return task._result
        end

        -- Yield from current task (for cooperative multitasking)
        function async.yield(value)
            return coroutine.yield(value)
        end

        -- Run multiple tasks concurrently (interleaved execution)
        function async.all(tasks)
            local results = {}
            local pending = {}

            for i, task in ipairs(tasks) do
                pending[i] = task
                results[i] = nil
            end

            -- Round-robin execution until all complete
            local any_pending = true
            while any_pending do
                any_pending = false
                for i, task in ipairs(pending) do
                    if task and coroutine.status(task._co) ~= "dead" then
                        any_pending = true
                        local ok, result = coroutine.resume(task._co)
                        if not ok then
                            error(result)
                        end
                        task._result = result
                    elseif task then
                        results[i] = task._result
                        task._completed = true
                        pending[i] = nil
                    end
                end
            end

            return results
        end

        -- Run tasks and return first completed result
        function async.race(tasks)
            while true do
                for i, task in ipairs(tasks) do
                    if coroutine.status(task._co) ~= "dead" then
                        local ok, result = coroutine.resume(task._co)
                        if not ok then
                            error(result)
                        end
                        if coroutine.status(task._co) == "dead" then
                            task._result = result
                            task._completed = true
                            return result, i
                        end
                    end
                end
            end
        end

        -- Sleep/delay (yields N times for cooperative scheduling)
        function async.sleep(n)
            for _ = 1, (n or 1) do
                coroutine.yield()
            end
        end

        return async
    "#;

    let async_module: Table = lua.load(async_code).eval()?;
    Ok(async_module)
}
