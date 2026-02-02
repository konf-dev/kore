const express = require('express');
const http = require('http');
const socketio = require('socket.io');

const app = express();
const server = http.createServer(app);
const io = socketio(server);

const PORT = process.env.PORT || 3000;

app.use(express.static('public'));

io.on('connection', (socket) => {
    console.log('A user connected:', socket.id);

    socket.on('disconnect', () => {
        console.log('User disconnected:', socket.id);
    });

    // Basic game state simulation for broadcasting
    socket.on('playerMove', (data) => {
        // In a real game, validate and update server-side game state
        // Then broadcast to other players
        socket.broadcast.emit('playerMoved', { id: socket.id, ...data });
    });

    socket.on('playerShoot', (data) => {
        socket.broadcast.emit('playerShot', { id: socket.id, ...data });
    });
});

server.listen(PORT, () => {
    console.log(`Server running on port ${PORT}`);
});
