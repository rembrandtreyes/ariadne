function callService(data) {
    return fetch('/api/handle', { method: 'POST', body: JSON.stringify(data) });
}
module.exports = { callService };
