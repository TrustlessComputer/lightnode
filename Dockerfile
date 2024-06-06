# Use a base image with Node.js 20 and Rust
FROM node:20-slim as node
FROM rust:1.67 as rust

# Install Protobuf compiler
RUN apt-get update && apt-get install -y protobuf-compiler

# Create a working directory
WORKDIR /app

# Copy the package.json and package-lock.json files
COPY services/package*.json ./

# Install Node.js dependencies
RUN npm ci

# Copy the source code
COPY services/ .

# Build the Rust project
COPY . .
RUN cargo build --release

# Set the environment variables
ENV PORT=5000
ENV VERIFY_SERVICE='cd ../ && cargo run -- reconstruct l1 --http-url https://rpc-amoy.polygon.technology/ --da-url https://rpc-amoy.polygon.technology/'

# Set the ulimit for open files
RUN ulimit -n 8192

# Expose the port
EXPOSE $PORT

# Start the Node.js server
WORKDIR /app/services 
CMD ["npm", "run", "dev"]