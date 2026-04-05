const { formatName } = require('../utils/helpers');

function getUsers(req, res) {
    res.json([]);
}

function createUser(req, res) {
    const name = formatName(req.body.name);
    res.json({ name });
}

module.exports = { getUsers, createUser };
