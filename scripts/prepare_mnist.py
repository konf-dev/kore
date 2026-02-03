#!/usr/bin/env python3
"""
MNIST Data Preparation for Kore

Downloads MNIST dataset and converts to JSON format for Kore training.
Creates smaller subsets for efficient training in Kore.

Usage:
    python prepare_mnist.py [--samples N] [--output DIR]
"""

import json
import struct
import gzip
import urllib.request
import os
import argparse
from pathlib import Path

MNIST_URLS = {
    "train_images": "https://ossci-datasets.s3.amazonaws.com/mnist/train-images-idx3-ubyte.gz",
    "train_labels": "https://ossci-datasets.s3.amazonaws.com/mnist/train-labels-idx1-ubyte.gz",
    "test_images": "https://ossci-datasets.s3.amazonaws.com/mnist/t10k-images-idx3-ubyte.gz",
    "test_labels": "https://ossci-datasets.s3.amazonaws.com/mnist/t10k-labels-idx1-ubyte.gz",
}

def download_file(url: str, path: Path) -> None:
    """Download file if not exists."""
    if path.exists():
        print(f"  {path.name} already exists, skipping download")
        return
    print(f"  Downloading {path.name}...")
    urllib.request.urlretrieve(url, path)

def read_images(path: Path) -> list:
    """Read MNIST images from gzipped IDX file."""
    with gzip.open(path, 'rb') as f:
        magic, num, rows, cols = struct.unpack('>IIII', f.read(16))
        assert magic == 2051, f"Invalid magic number: {magic}"
        data = f.read()
        images = []
        for i in range(num):
            start = i * rows * cols
            end = start + rows * cols
            # Normalize to [0, 1]
            pixels = [b / 255.0 for b in data[start:end]]
            images.append(pixels)
        return images

def read_labels(path: Path) -> list:
    """Read MNIST labels from gzipped IDX file."""
    with gzip.open(path, 'rb') as f:
        magic, num = struct.unpack('>II', f.read(8))
        assert magic == 2049, f"Invalid magic number: {magic}"
        return list(f.read())

def main():
    parser = argparse.ArgumentParser(description="Prepare MNIST data for Kore")
    parser.add_argument("--samples", type=int, default=1000, 
                        help="Number of training samples (default: 1000)")
    parser.add_argument("--test-samples", type=int, default=200,
                        help="Number of test samples (default: 200)")
    parser.add_argument("--output", type=str, default="data",
                        help="Output directory (default: data)")
    args = parser.parse_args()
    
    output_dir = Path(args.output)
    output_dir.mkdir(exist_ok=True)
    cache_dir = output_dir / "cache"
    cache_dir.mkdir(exist_ok=True)
    
    print("Downloading MNIST dataset...")
    files = {}
    for name, url in MNIST_URLS.items():
        path = cache_dir / f"{name}.gz"
        download_file(url, path)
        files[name] = path
    
    print("\nReading training data...")
    train_images = read_images(files["train_images"])
    train_labels = read_labels(files["train_labels"])
    print(f"  Loaded {len(train_images)} training images")
    
    print("Reading test data...")
    test_images = read_images(files["test_images"])
    test_labels = read_labels(files["test_labels"])
    print(f"  Loaded {len(test_images)} test images")
    
    # Create subset for Kore training
    n_train = min(args.samples, len(train_images))
    n_test = min(args.test_samples, len(test_images))
    
    print(f"\nCreating training subset ({n_train} samples)...")
    train_data = {
        "images": train_images[:n_train],
        "labels": train_labels[:n_train],
        "n_samples": n_train,
        "n_features": 784,
        "n_classes": 10,
    }
    
    print(f"Creating test subset ({n_test} samples)...")
    test_data = {
        "images": test_images[:n_test],
        "labels": test_labels[:n_test],
        "n_samples": n_test,
        "n_features": 784,
        "n_classes": 10,
    }
    
    # Save as JSON
    train_path = output_dir / "mnist_train.json"
    test_path = output_dir / "mnist_test.json"
    
    print(f"\nSaving {train_path}...")
    with open(train_path, 'w') as f:
        json.dump(train_data, f)
    print(f"  Size: {train_path.stat().st_size / 1024:.1f} KB")
    
    print(f"Saving {test_path}...")
    with open(test_path, 'w') as f:
        json.dump(test_data, f)
    print(f"  Size: {test_path.stat().st_size / 1024:.1f} KB")
    
    # Also save a tiny sample for quick testing
    tiny_data = {
        "images": train_images[:10],
        "labels": train_labels[:10],
        "n_samples": 10,
        "n_features": 784,
        "n_classes": 10,
    }
    tiny_path = output_dir / "mnist_tiny.json"
    print(f"Saving {tiny_path} (10 samples for quick testing)...")
    with open(tiny_path, 'w') as f:
        json.dump(tiny_data, f)
    
    print("\n✓ MNIST data prepared successfully!")
    print(f"\nUsage in Kore:")
    print(f'  "data/mnist_train.json" fs-read json-parse')
    print(f'  dup "images" map-get 0 list-get tensor-from-list  # First image')
    print(f'  swap "labels" map-get 0 list-get                   # First label')

if __name__ == "__main__":
    main()
