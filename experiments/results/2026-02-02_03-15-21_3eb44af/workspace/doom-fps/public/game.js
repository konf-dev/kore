// Basic game setup
const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');

canvas.width = window.innerWidth;
canvas.height = window.innerHeight;

// Player settings
const player = {
    x: canvas.width / 2,
    y: canvas.height / 2,
    angle: Math.PI / 2, // Facing right
    fov: Math.PI / 3, // Field of view
    speed: 5,
    rotationSpeed: 0.05,
};

// Map (simple representation)
const map = [
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
];

const TILE_SIZE = 64;

// Raycasting engine (simplified)
function draw3DView() {
    for (let i = 0; i < canvas.width; i++) {
        const rayAngle = player.angle - player.fov / 2 + (i / canvas.width) * player.fov;
        let wallHit = false;
        let dist = 0;

        for (let j = 0; j < 500; j++) { // Max ray distance
            const testX = player.x + Math.cos(rayAngle) * j;
            const testY = player.y + Math.sin(rayAngle) * j;

            const mapX = Math.floor(testX / TILE_SIZE);
            const mapY = Math.floor(testY / TILE_SIZE);

            if (map[mapY] && map[mapY][mapX] === 1) {
                wallHit = true;
                dist = j;
                break;
            }
        }

        if (wallHit) {
            const wallHeight = (TILE_SIZE * canvas.height) / (dist * Math.cos(rayAngle - player.angle));
            const textureX = testX % TILE_SIZE;

            ctx.fillStyle = `rgb(${255 - dist * 0.1}, ${255 - dist * 0.1}, ${255 - dist * 0.1})`;
            ctx.fillRect(i, canvas.height / 2 - wallHeight / 2, 1, wallHeight);

            // Ceiling
            ctx.fillStyle = '#ADD8E6'; // Light blue
            ctx.fillRect(i, 0, 1, canvas.height / 2 - wallHeight / 2);

            // Floor
            ctx.fillStyle = '#8B4513'; // Brown
            ctx.fillRect(i, canvas.height / 2 + wallHeight / 2, 1, canvas.height / 2 - wallHeight / 2);
        }
    }
}

// Drawing minimap for debugging (optional, can be removed)
function drawMinimap() {
    const minimapScale = 0.2;
    ctx.save();
    ctx.scale(minimapScale, minimapScale);

    for (let y = 0; y < map.length; y++) {
        for (let x = 0; x < map[y].length; x++) {
            if (map[y][x] === 1) {
                ctx.fillStyle = 'gray';
                ctx.fillRect(x * TILE_SIZE, y * TILE_SIZE, TILE_SIZE, TILE_SIZE);
            }
        }
    }

    // Draw player on minimap
    ctx.fillStyle = 'red';
    ctx.beginPath();
    ctx.arc(player.x, player.y, 10, 0, Math.PI * 2);
    ctx.fill();

    // Draw player direction
    ctx.strokeStyle = 'red';
    ctx.beginPath();
    ctx.moveTo(player.x, player.y);
    ctx.lineTo(player.x + Math.cos(player.angle) * 50,
               player.y + Math.sin(player.angle) * 50);
    ctx.stroke();

    ctx.restore();
}

// Update function
function update() {
    // Movement logic (simplified)
    if (keys['w']) {
        player.x += Math.cos(player.angle) * player.speed;
        player.y += Math.sin(player.angle) * player.speed;
    }
    if (keys['s']) {
        player.x -= Math.cos(player.angle) * player.speed;
        player.y -= Math.sin(player.angle) * player.speed;
    }
    if (keys['a']) {
        player.angle -= player.rotationSpeed;
    }
    if (keys['d']) {
        player.angle += player.rotationSpeed;
    }

    // Basic collision detection (prevent walking through walls)
    const playerMapX = Math.floor(player.x / TILE_SIZE);
    const playerMapY = Math.floor(player.y / TILE_SIZE);
    if (map[playerMapY] && map[playerMapY][playerMapX] === 1) {
        // Rudimentary collision: push player back
        if (keys['w']) {
            player.x -= Math.cos(player.angle) * player.speed;
            player.y -= Math.sin(player.angle) * player.speed;
        }
        if (keys['s']) {
            player.x += Math.cos(player.angle) * player.speed;
            player.y += Math.sin(player.angle) * player.speed;
        }
    }

    // Render
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    draw3DView();
    // drawMinimap(); // Uncomment for minimap debugging
}

// Input handling
const keys = {};
window.addEventListener('keydown', (e) => {
    keys[e.key.toLowerCase()] = true;
});
window.addEventListener('keyup', (e) => {
    keys[e.key.toLowerCase()] = false;
});

// Mouse look (simplified)
// let mouseX = 0;
// window.addEventListener('mousemove', (e) => {
//     if (document.pointerLockElement === canvas) {
//         player.angle += e.movementX * 0.002; // Adjust sensitivity
//     }
// });

// canvas.addEventListener('click', () => {
//     canvas.requestPointerLock();
// });

// Socket.IO for multiplayer (placeholder)
const socket = io();
socket.on('connect', () => {
    console.log('Connected to server');
});

socket.on('playerMoved', (data) => {
    // Handle other players' movements
    console.log('Player moved:', data);
});

// Game loop
function gameLoop() {
    update();
    requestAnimationFrame(gameLoop);
}

gameLoop();
