const { greet } = require('./utils');

function main() {
    greet('World');
}

class ApiClient {
    fetch(url) {
        return fetch(url);
    }
}

module.exports = { main, ApiClient };
