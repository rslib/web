//! Coroutine helpers for Lua API
//!
//! Functions: coro.task, coro.await, coro.yield, coro.all, coro.race, coro.sleep

use mlua::{Lua, Result, Table};

/// Create the coro module table
pub fn create_module(lua: &Lua) -> Result<Table> {
    // Create coro module using Lua code
    let coro_code = r#"
        local coro = {}

        -- Create a task from a function (wraps in coroutine)
        function coro.task(fn)
            return {
                _co = coroutine.create(fn),
                _completed = false,
                _result = nil,
            }
        end

        -- Run a task to completion
        function coro.await(task)
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
        function coro.yield(value)
            return coroutine.yield(value)
        end

        -- Run multiple tasks concurrently (interleaved execution)
        function coro.all(tasks)
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
        function coro.race(tasks)
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
        function coro.sleep(n)
            for _ = 1, (n or 1) do
                coroutine.yield()
            end
        end

        return coro
    "#;

    let coro_module: Table = lua.load(coro_code).eval()?;
    Ok(coro_module)
}
