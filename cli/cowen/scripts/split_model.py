#!/usr/bin/env python3
import os
import sys

def split_file(input_file, chunks_dir, chunk_size=2 * 1024 * 1024):
    if not os.path.exists(input_file):
        print(f"Warning: File not found for splitting: {input_file}")
        return False

    base_name = os.path.basename(input_file)
    print(f"Splitting '{input_file}' into chunks of 2MB under '{chunks_dir}'...")

    # Clean up existing chunks for this specific file first
    if os.path.exists(chunks_dir):
        for f in os.listdir(chunks_dir):
            if f.startswith(f"{base_name}.chunk"):
                os.remove(os.path.join(chunks_dir, f))
    else:
        os.makedirs(chunks_dir, exist_ok=True)

    file_size = os.path.getsize(input_file)
    chunk_idx = 0

    with open(input_file, 'rb') as f:
        while True:
            data = f.read(chunk_size)
            if not data:
                break
            chunk_filename = f"{base_name}.chunk{chunk_idx:02d}"
            chunk_path = os.path.join(chunks_dir, chunk_filename)
            with open(chunk_path, 'wb') as chunk_f:
                chunk_f.write(data)
            print(f"  Created chunk {chunk_idx:02d}: {chunk_filename} ({len(data)} bytes)")
            chunk_idx += 1

    print(f"Successfully split '{input_file}' ({file_size} bytes) into {chunk_idx} chunks.\n")
    return True

def main():
    # Find repository root
    script_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = None
    
    curr = script_dir
    while True:
        if os.path.exists(os.path.join(curr, "assets", "search", "models")):
            root_dir = curr
            break
        parent = os.path.dirname(curr)
        if parent == curr:
            break
        curr = parent
        
    if not root_dir:
        print("Error: Could not find repository root directory containing assets/search/models", file=sys.stderr)
        sys.exit(1)
        
    models_dir = os.path.join(root_dir, "assets", "search", "models")
    chunks_dir = os.path.join(models_dir, "chunks")
    
    # 2MB chunks
    chunk_size = 2 * 1024 * 1024
    
    # Split model_quantized.onnx
    onnx_file = os.path.join(models_dir, "model_quantized.onnx")
    split_file(onnx_file, chunks_dir, chunk_size)
    
    # Split model_quantized.onnx_data (if exists)
    onnx_data_file = os.path.join(models_dir, "model_quantized.onnx_data")
    split_file(onnx_data_file, chunks_dir, chunk_size)

if __name__ == '__main__':
    main()
