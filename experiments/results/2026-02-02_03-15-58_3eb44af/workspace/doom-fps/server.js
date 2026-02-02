const express = require('express');
const http = require('http');
const socketIo = require('socket.io');

const app = express();
const server = http.createServer(app);
const io = socketIo(server);

const PORT = process.env.PORT || 3000;

// Serve static files from the 'public' directory
app.use(express.static('doom-fps/public'));

// Basic route for the root
app.get('/', (req, res) => {
  res.sendFile(__dirname + '/public/index.html');
});

io.on('connection', (socket) => {
  console.log('a user connected');

  socket.on('disconnect', () => {
    console.log('user disconnected');
  });

  // Example: Echo messages
  socket.on('chat message', (msg) => {
    io.emit('chat message', msg); // Broadcast to all clients
  });
});

server.listen(PORT, () => {
  console.log(`Server listening on *:${PORT}`);
});