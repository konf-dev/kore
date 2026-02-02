const express = require('express');
const http = require('http');
const socketIo = require('socket.io');

const app = express();
const server = http.createServer(app);
const io = socketIo(server);

const PORT = process.env.PORT || 3000;

// Serve static files from the 'public' directory
app.use(express.static('public'));

app.get('/', (req, res) => {
  res.sendFile(__dirname + '/public/index.html');
});

io.on('connection', (socket) => {
  console.log('A user connected:', socket.id);

  socket.on('disconnect', () => {
    console.log('User disconnected:', socket.id);
  });

  // Handle player movement, shooting, etc.
  socket.on('playerMove', (data) => {
    // console.log('Player move:', data);
    // Broadcast to other players
    socket.broadcast.emit('playerMove', { id: socket.id, ...data });
  });

  socket.on('playerShoot', (data) => {
    // console.log('Player shoot:', data);
    socket.broadcast.emit('playerShoot', { id: socket.id, ...data });
  });

  // Initial state synchronization (for new players)
  // In a real game, you would send current game state to the new player
  socket.emit('gameState', { message: 'Welcome to the game!' });
});

server.listen(PORT, () => {
  console.log(`Server listening on port ${PORT}`);
});
