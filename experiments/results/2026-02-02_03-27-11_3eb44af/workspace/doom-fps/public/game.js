// Game setup
const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');
canvas.width = window.innerWidth;
canvas.height = window.innerHeight;

// Socket.IO for multiplayer
const socket = io();

// Game state (placeholder)
let players = {};
let walls = [
    { x1: 50, y1: 50, x2: 250, y2: 50, color: 'red' },
    { x1: 250, y1: 50, x2: 250, y2: 250, color: 'blue' },
    { x1: 250, y1: 250, x2: 50, y2: 250, color: 'green' },
    { x1: 50, y1: 250, x2: 50, y2: 50, color: 'yellow' }
];

let player = {
    x: 100,
    y: 100,
    angle: 0, // Radians
    fov: Math.PI / 3, // Field of View
    rotationSpeed: 0.05,
    walkSpeed: 2
};

// Raycasting engine (simplified placeholder)
function drawWalls() {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    // Draw floor and ceiling (simple color bands for now)
    ctx.fillStyle = '#666'; // Floor color
    ctx.fillRect(0, canvas.height / 2, canvas.width, canvas.height / 2);
    ctx.fillStyle = '#333'; // Ceiling color
    ctx.fillRect(0, 0, canvas.width, canvas.height / 2);

    // Simplified raycasting drawing
    const numRays = canvas.width;
    const angleStep = player.fov / numRays;

    for (let i = 0; i < numRays; i++) {
        const rayAngle = player.angle - player.fov / 2 + i * angleStep; 
        
        // Instead of actual ray-wall intersection, draw vertical lines for walls
        // This is a gross simplification for a placeholder
        const wallHeight = 100; // Placeholder height
        const wallSliceHeight = (canvas.height / wallHeight) * (canvas.height / 2) / Math.cos(rayAngle - player.angle); // Perspective hack
        const wallY = (canvas.height / 2) - (wallSliceHeight / 2);

        ctx.fillStyle = `hsl(${i % 360}, 70%, 50%)`; // Vary color for visual effect
        ctx.fillRect(i, wallY, 1, wallSliceHeight);
    }
}

// Player movement
document.addEventListener('keydown', (e) => {
    if (e.key === 'w') { player.x += Math.cos(player.angle) * player.walkSpeed; player.y += Math.sin(player.angle) * player.walkSpeed; }
    if (e.key === 's') { player.x -= Math.cos(player.angle) * player.walkSpeed; player.y -= Math.sin(player.angle) * player.walkSpeed; }
    if (e.key === 'a') { player.angle -= player.rotationSpeed; }
    if (e.key === 'd') { player.angle += player.rotationSpeed; }
    // Emit player position to server
    socket.emit('playerMove', { x: player.x, y: player.y, angle: player.angle });
});

// Mouse look (simplistic)
// canvas.addEventListener('mousemove', (e) => {
//     player.angle += e.movementX * 0.005;
// });

// Game loop
function gameLoop() {
    drawWalls();
    // Other drawing: player, enemies, sprites
    requestAnimationFrame(gameLoop);
}

// Start game
gameLoop();

// Socket.IO event listeners (placeholder)
socket.on('connect', () => {
    console.log('Connected to server');
    socket.emit('newPlayer', { id: socket.id, x: player.x, y: player.y, angle: player.angle });
});

socket.on('currentPlayers', (serverPlayers) => {
    players = serverPlayers;
    console.log('Current players:', players);
});

socket.on('playerMoved', (movedPlayer) => {
    players[movedPlayer.id] = movedPlayer;
});

socket.on('playerDisconnected', (playerId) => {
    delete players[playerId];
});

socket.on('disconnect', () => {
    console.log('Disconnected from server');
});

// Placeholder for shooting mechanics
function shoot() {
    console.log('BANG!');
    socket.emit('shoot', { x: player.x, y: player.y, angle: player.angle });
}

document.addEventListener('click', shoot);
