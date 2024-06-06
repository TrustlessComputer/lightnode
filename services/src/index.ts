import dotenv from 'dotenv';
import express, { Request, Response, NextFunction } from 'express';
import bodyParser from 'body-parser';
import routes from './routes';
import { startVerifyService } from './services/VerifyService';

// Load environment variables from .env file
dotenv.config();

const app = express();
const port = process.env.PORT || 5000;
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE');
  res.header('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  next();
});
// Middleware to parse JSON request bodies
app.use(bodyParser.json());

// Use routes
app.use('/api', routes);
app.use('/status', routes);

// Error handling middleware
app.use((err: Error, req: Request, res: Response, next: NextFunction) => {
  console.error(err.stack);
  res.status(500).send('Something broke!');
});

// Start the server
app.listen(port, () => {
  console.log(`Server is running on http://localhost:${port}`);
  const cmd = process.env.VERIFY_SERVICE;
  startVerifyService(cmd!);
});