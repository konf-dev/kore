#!/usr/bin/env python3
"""
MNIST Neural Network in Python
For comparison with Kore implementation

Simple 2-layer network: 784 -> 128 -> 10
Uses only numpy, no deep learning frameworks
"""

import numpy as np
import time
import struct
import gzip
from pathlib import Path

# ============================================
# Data Loading
# ============================================

def load_mnist_images(filename):
    """Load MNIST images from IDX file"""
    with gzip.open(filename, 'rb') as f:
        magic, num, rows, cols = struct.unpack('>IIII', f.read(16))
        images = np.frombuffer(f.read(), dtype=np.uint8)
        images = images.reshape(num, rows * cols)
        return images.astype(np.float32) / 255.0

def load_mnist_labels(filename):
    """Load MNIST labels from IDX file"""
    with gzip.open(filename, 'rb') as f:
        magic, num = struct.unpack('>II', f.read(8))
        labels = np.frombuffer(f.read(), dtype=np.uint8)
        return labels

def one_hot(labels, num_classes=10):
    """Convert labels to one-hot encoding"""
    return np.eye(num_classes)[labels]

def download_mnist():
    """Download MNIST if not present"""
    import urllib.request
    base_url = "http://yann.lecun.com/exdb/mnist/"
    files = [
        "train-images-idx3-ubyte.gz",
        "train-labels-idx1-ubyte.gz",
        "t10k-images-idx3-ubyte.gz",
        "t10k-labels-idx1-ubyte.gz"
    ]
    data_dir = Path(__file__).parent / "data"
    data_dir.mkdir(exist_ok=True)
    
    for f in files:
        path = data_dir / f
        if not path.exists():
            print(f"Downloading {f}...")
            urllib.request.urlretrieve(base_url + f, path)
    return data_dir

# ============================================
# Activation Functions
# ============================================

def sigmoid(x):
    return 1.0 / (1.0 + np.exp(-np.clip(x, -500, 500)))

def sigmoid_derivative(s):
    """Given sigmoid output s, return derivative"""
    return s * (1 - s)

def relu(x):
    return np.maximum(0, x)

def relu_derivative(x):
    return (x > 0).astype(np.float32)

def softmax(x):
    exp_x = np.exp(x - np.max(x, axis=1, keepdims=True))
    return exp_x / np.sum(exp_x, axis=1, keepdims=True)

# ============================================
# Neural Network
# ============================================

class NeuralNetwork:
    def __init__(self, input_size=784, hidden_size=128, output_size=10):
        # Xavier initialization
        self.W1 = np.random.randn(input_size, hidden_size) * np.sqrt(2.0 / input_size)
        self.b1 = np.zeros((1, hidden_size))
        self.W2 = np.random.randn(hidden_size, output_size) * np.sqrt(2.0 / hidden_size)
        self.b2 = np.zeros((1, output_size))
        
    def forward(self, X):
        """Forward propagation"""
        self.z1 = X @ self.W1 + self.b1
        self.a1 = relu(self.z1)
        self.z2 = self.a1 @ self.W2 + self.b2
        self.a2 = softmax(self.z2)
        return self.a2
    
    def backward(self, X, y, learning_rate=0.01):
        """Backpropagation"""
        m = X.shape[0]
        
        # Output layer gradient
        dz2 = self.a2 - y  # Cross-entropy + softmax derivative
        dW2 = (self.a1.T @ dz2) / m
        db2 = np.sum(dz2, axis=0, keepdims=True) / m
        
        # Hidden layer gradient
        da1 = dz2 @ self.W2.T
        dz1 = da1 * relu_derivative(self.z1)
        dW1 = (X.T @ dz1) / m
        db1 = np.sum(dz1, axis=0, keepdims=True) / m
        
        # Update weights
        self.W2 -= learning_rate * dW2
        self.b2 -= learning_rate * db2
        self.W1 -= learning_rate * dW1
        self.b1 -= learning_rate * db1
        
    def predict(self, X):
        return np.argmax(self.forward(X), axis=1)
    
    def accuracy(self, X, y_labels):
        predictions = self.predict(X)
        return np.mean(predictions == y_labels)
    
    def cross_entropy_loss(self, y_pred, y_true):
        m = y_true.shape[0]
        epsilon = 1e-15
        return -np.sum(y_true * np.log(y_pred + epsilon)) / m

# ============================================
# Training
# ============================================

def train(epochs=10, batch_size=64, learning_rate=0.1, subset_size=None):
    """Train the network and return metrics"""
    
    # Try to load MNIST
    try:
        data_dir = download_mnist()
        X_train = load_mnist_images(data_dir / "train-images-idx3-ubyte.gz")
        y_train_labels = load_mnist_labels(data_dir / "train-labels-idx1-ubyte.gz")
        X_test = load_mnist_images(data_dir / "t10k-images-idx3-ubyte.gz")
        y_test_labels = load_mnist_labels(data_dir / "t10k-labels-idx1-ubyte.gz")
    except Exception as e:
        print(f"Could not load MNIST: {e}")
        print("Using synthetic data for testing...")
        # Generate synthetic data for testing
        np.random.seed(42)
        X_train = np.random.randn(1000, 784).astype(np.float32) * 0.3
        y_train_labels = np.random.randint(0, 10, 1000)
        X_test = np.random.randn(200, 784).astype(np.float32) * 0.3
        y_test_labels = np.random.randint(0, 10, 200)
    
    # Use subset for faster testing
    if subset_size:
        X_train = X_train[:subset_size]
        y_train_labels = y_train_labels[:subset_size]
        X_test = X_test[:min(subset_size // 5, len(X_test))]
        y_test_labels = y_test_labels[:min(subset_size // 5, len(y_test_labels))]
    
    y_train = one_hot(y_train_labels)
    
    print(f"Training samples: {len(X_train)}")
    print(f"Test samples: {len(X_test)}")
    print(f"Epochs: {epochs}, Batch size: {batch_size}, LR: {learning_rate}")
    print("-" * 50)
    
    # Initialize network
    nn = NeuralNetwork()
    
    # Training loop
    start_time = time.time()
    
    for epoch in range(epochs):
        epoch_start = time.time()
        
        # Shuffle data
        indices = np.random.permutation(len(X_train))
        X_shuffled = X_train[indices]
        y_shuffled = y_train[indices]
        
        # Mini-batch training
        for i in range(0, len(X_train), batch_size):
            X_batch = X_shuffled[i:i+batch_size]
            y_batch = y_shuffled[i:i+batch_size]
            
            nn.forward(X_batch)
            nn.backward(X_batch, y_batch, learning_rate)
        
        # Compute metrics
        train_pred = nn.forward(X_train)
        train_loss = nn.cross_entropy_loss(train_pred, y_train)
        train_acc = nn.accuracy(X_train, y_train_labels)
        test_acc = nn.accuracy(X_test, y_test_labels)
        epoch_time = time.time() - epoch_start
        
        print(f"Epoch {epoch+1:2d}: loss={train_loss:.4f}, "
              f"train_acc={train_acc:.4f}, test_acc={test_acc:.4f}, "
              f"time={epoch_time:.3f}s")
    
    total_time = time.time() - start_time
    final_test_acc = nn.accuracy(X_test, y_test_labels)
    
    print("-" * 50)
    print(f"Total training time: {total_time:.3f}s")
    print(f"Final test accuracy: {final_test_acc:.4f}")
    
    return {
        'epochs': epochs,
        'batch_size': batch_size,
        'learning_rate': learning_rate,
        'total_time': total_time,
        'final_accuracy': final_test_acc,
        'samples_trained': len(X_train),
    }

# ============================================
# Main
# ============================================

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description='Train MNIST with pure numpy')
    parser.add_argument('--epochs', type=int, default=10)
    parser.add_argument('--batch-size', type=int, default=64)
    parser.add_argument('--lr', type=float, default=0.1)
    parser.add_argument('--subset', type=int, default=None, 
                        help='Use subset of data for quick testing')
    args = parser.parse_args()
    
    print("=" * 50)
    print("MNIST Neural Network - Python/NumPy")
    print("=" * 50)
    
    results = train(
        epochs=args.epochs,
        batch_size=args.batch_size,
        learning_rate=args.lr,
        subset_size=args.subset
    )
