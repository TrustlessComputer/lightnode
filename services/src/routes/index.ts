import { Router, Request, Response } from 'express';
import statusRoutes from './status';
import chainRoutes from './chain';
const router = Router();

// Define a simple route
router.get('/', (req: Request, res: Response) => {
  res.send('Welcome to the API!');
});

// Use the /status route
router.use(statusRoutes);
router.use(chainRoutes)
export default router;