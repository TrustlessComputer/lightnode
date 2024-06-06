import { Router, Request, Response } from 'express';
import statusRoutes from './status';

const router = Router();

// Define a simple route
router.get('/', (req: Request, res: Response) => {
  res.send('Welcome to the API!');
});

// Example route for handling data
router.post('/data', (req: Request, res: Response) => {
  const data = req.body;
  res.send(`You sent: ${JSON.stringify(data)}`);
});

// Use the /status route
router.use(statusRoutes);

export default router;