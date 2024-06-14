# Use a base image with Node.js 20 and Rust
FROM node:20-bookworm as node
WORKDIR /app
COPY . .

RUN cd services && npm ci

# Install Protobuf compiler  
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates protobuf-compiler
RUN update-ca-certificates
RUN cd services && npm run build

# Set the ulimit for open files
RUN ulimit -n 8192

# Expose the port
EXPOSE 5000

# Start the Node.js server
WORKDIR /app/services
CMD ["npm", "run", "prod"]
