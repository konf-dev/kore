const express = require('express');
const http = require('http');
const socketIo = require('socket.io');

const app = express();
const server = http.createServer(app);
const io = socketIo(server);

const PORT = process.env.PORT || 3000;

app.use(express.static('public'));

let players = {}; // Store player data

io.on('connection', (socket) => {
    console.log('A user connected:', socket.id);

    // Initialize new player
    players[socket.id] = {
        id: socket.id,
        x: Math.random() * 5,
        y: Math.random() * 5,
        angle: 0,
        health: 100
    };

    // Send existing players to the new player
    socket.emit('currentPlayers', players);
    // Broadcast new player to all other players
    socket.broadcast.emit('newPlayer', players[socket.id]);

    socket.on('playerMovement', (movementData) => {
        if (players[socket.id]) {
            players[socket.id].x = movementData.x;
            players[socket.id].y = movementData.y;
            players[socket.id].angle = movementData.angle;
            // Broadcast updated position to all clients
            socket.broadcast.emit('playerMoved', players[socket.id]);
        }
    });

    socket.on('disconnect', () => {
        console.log('User disconnected:', socket.id);
        delete players[socket.id];
        // Broadcast removal to all clients
        io.emit('playerDisconnected', socket.id);
    });
});

server.listen(PORT, () => {
    console.log(`Listening on *:${PORT}`);
});
