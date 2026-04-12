// Fixture for testing module-level call tracking.
// setup() and teardown() are defined here; calls to them happen at module
// scope (outside any function body), which means the parser must emit them
// with caller_name = "<module>".

function setup() {
    return 'ready';
}

function teardown() {
    return 'done';
}

// Module-level calls — must be tracked from the synthetic <module> caller
setup();
Promise.allSettled([teardown()]);
