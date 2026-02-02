const express = require('express');
const app = express();
const port = 3001; // Use a different port for backend

app.get('/api/data', (req, res) => {
  res.json({ message: 'Hello from Backend!' });
});

app.listen(port, () => {
  console.log(`Backend server listening at http://localhost:${port}`);
});
