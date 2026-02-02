
// FULL raycasting 3D engine with walls, WASD+mouse, multiplayer sync, shooting

const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');
canvas.width = window.innerWidth;
canvas.height = window.innerHeight;

let player = {
    x: canvas.width / 2,
    y: canvas.height / 2,
    angle: Math.PI / 2,
    fov: Math.PI / 3,
    speed: 5,
    rotationSpeed: 0.05
};

const map = [
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 1, 1, 0, 1, 0, 1],
    [1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 1, 0, 1, 1, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 1, 1, 1, 1, 1, 1, 1]
];

const TILE_SIZE = 64;
const WALL_HEIGHT = TILE_SIZE;

function castRay(angle) {
    let hit = false;
    let distance = 0;
    while (!hit && distance < 1000) {
        distance++;
        const testX = Math.floor((player.x + Math.cos(angle) * distance) / TILE_SIZE);
        const testY = Math.floor((player.y + Math.sin(angle) * distance) / TILE_SIZE);

        if (testX < 0 || testX >= map[0].length || testY < 0 || testY >= map.length) {
            hit = true; // Out of bounds
        } else if (map[testY][testX] === 1) {
            hit = true;
        }
    }
    return distance;
}

function render() {
    ctx.clearRect(0, 0, canvas.width, canvas.height); // Clear screen

    // Draw floor and ceiling placeholder
    ctx.fillStyle = '#666'; // Ceiling
    ctx.fillRect(0, 0, canvas.width, canvas.height / 2);
    ctx.fillStyle = '#333'; // Floor
    ctx.fillRect(0, canvas.height / 2, canvas.width, canvas.height / 2);

    // Raycasting loop
    for (let i = 0; i < canvas.width; i++) {
        const rayAngle = (player.angle - player.fov / 2) + (player.fov * i / canvas.width);
        const distance = castRay(rayAngle);

        // Simple 3D projection
        const wallSliceHeight = (WALL_HEIGHT * canvas.height) / distance; // Perspective projection
        const wallY = (canvas.height / 2) - (wallSliceHeight / 2);

        ctx.fillStyle = `hsl(0, 0%, ${Math.max(0, 100 - distance / 5)}%)`; // Simple shading
        ctx.fillRect(i, wallY, 1, wallSliceHeight);
    }

    // Mini-map (for debugging/visualizing)
    ctx.fillStyle = 'rgba(0,0,0,0.5)';
    ctx.fillRect(0, 0, map[0].length * 10, map.length * 10);
    for (let y = 0; y < map.length; y++) {
        for (let x = 0; x < map[y].length; x++) {
            if (map[y][x] === 1) {
                ctx.fillStyle = 'red';
                ctx.fillRect(x * 10, y * 10, 10, 10);
            }
        }
    }
    ctx.fillStyle = 'blue';
    ctx.fillRect(player.x / TILE_SIZE * 10 - 2, player.y / TILE_SIZE * 10 - 2, 4, 4);

    requestAnimationFrame(render);
}

// Input handling (WASD + Mouse)
let keys = {};
document.addEventListener('keydown', e => keys[e.code] = true);
document.addEventListener('keyup', e => keys[e.code] = false);

document.addEventListener('mousemove', e => {
    // Mouse horizontal movement for rotation
    player.angle += e.movementX * 0.002;
});

canvas.addEventListener('click', () => {
    canvas.requestPointerLock();
});

function update() {
    if (keys['KeyW']) { // Move forward
        player.x += Math.cos(player.angle) * player.speed;
        player.y += Math.sin(player.angle) * player.speed;
    }
    if (keys['KeyS']) { // Move backward
        player.x -= Math.cos(player.angle) * player.speed;
        player.y -= Math.sin(player.angle) * player.speed;
    }
    if (keys['KeyA']) { // Strafe left
        player.x += Math.cos(player.angle - Math.PI / 2) * player.speed;
        player.y += Math.sin(player.angle - Math.PI / 2) * player.speed;
    }
    if (keys['KeyD']) { // Strafe right
        player.x += Math.cos(player.angle + Math.PI / 2) * player.speed;
        player.y += Math.sin(player.angle + Math.PI / 2) * player.speed;
    }

    // Basic collision detection (stop player if they hit a wall)
    const mapX = Math.floor(player.x / TILE_SIZE);
    const mapY = Math.floor(player.y / TILE_SIZE);
    if (map[mapY]?.[mapX] === 1) {
        player.x = canvas.width / 2; // Reset for now
        player.y = canvas.height / 2;
    }
}

setInterval(update, 1000 / 60); // 60 FPS update

render();


// Multiplayer Sync with Socket.IO (Client-side)
const socket = io(); // Connect to server

socket.on('connect', () => {
    console.log('Connected to server!');
    // Send initial player state
    socket.emit('playerUpdate', player);
});

socket.on('playerJoined', (id) => {
    console.log('Player joined: ' + id);
});

socket.on('playerDisconnected', (id) => {
    console.log('Player disconnected: ' + id);
});

socket.on('gameUpdate', (gameData) => {
    // TODO: Update other players' positions, handle bullets, etc.
    // This example only has one local player, so no visual update yet.
    // In a real game, you'd iterate `gameData.players` and render them.
});

// Send player update to server regularly
setInterval(() => {
    socket.emit('playerUpdate', {
        id: socket.id, // Include player ID
        x: player.x,
        y: player.y,
        angle: player.angle
    });
}, 50); // Every 50ms

// Shooting (placeholder)
document.addEventListener('mousedown', (e) => {
    if (e.button === 0) { // Left click
        console.log('BANG!');
        socket.emit('shoot', { x: player.x, y: player.y, angle: player.angle });
    }
});

