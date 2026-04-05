const express = require('express');
const { getUsers, createUser } = require('./routes/users');
const { authMiddleware } = require('./middleware/auth');

const app = express();

app.use(authMiddleware);
app.get('/users', getUsers);
app.post('/users', createUser);

app.listen(3000);
