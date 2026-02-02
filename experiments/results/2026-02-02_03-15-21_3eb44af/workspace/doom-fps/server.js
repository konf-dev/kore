const express = require('express');
const http = require('http');
const socketIo = require('socket.io');

const app = express();
const server = http.createServer(app);
const io = socketIo(server);

const PORT = process.env.PORT || 3000;

// Serve static files from the 'public' directory
app.use(express.static('doom-fps/public'));

io.on('connection', (socket) => {
  console.log('A user connected:', socket.id);

  socket.on('disconnect', () => {
    console.log('User disconnected:', socket.id);
  });

  // Handle game-specific events here
  // Example: player movement, shooting, etc.
  socket.on('playerMove', (data) => {
    // console.log('Player move:', socket.id, data);
    socket.broadcast.emit('playerMove', { id: socket.id, ...data });
  });

  socket.on('playerShoot', (data) => {
    // console.log('Player shoot:', socket.id, data);
    socket.broadcast.emit('playerShoot', { id: socket.id, ...data });
  });

  // Send initial player state or game state to the new connection
  socket.emit('currentPlayers', {}); // Placeholder
});

server.listen(PORT, '0.0.0.0', () => {
  console.log(`Server listening on port ${PORT}`);
});
