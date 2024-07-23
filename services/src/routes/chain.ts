import { Router, Request, Response } from 'express';
import axios from 'axios';

const router = Router();


// POST endpoint to receive txid
router.post('/getTxDetails', async (req, res) => {
    const url = process.env.CHAIN_URL;
    const { txid } = req.body;
    
    if (!txid) {
      return res.status(400).send({ error: 'txid is required' });
    }
  
    try {
      // Call the JSON-RPC method
      const response = await axios.post(url!, {
        jsonrpc: "2.0",
        id: 1,
        method: "zks_getTransactionDetails",
        params: [txid]
      }, {
        headers: {
          'Content-Type': 'application/json'
        }
      });
  
      // Return the result
      res.send(response.data);
    } catch (error) {
      console.error(error);
      res.status(500).send({ error: 'Failed to fetch transaction details' });
    }
  });

// POST get blocks in batch
router.post('/getL1BatchBlockRange', async (req, res) => {
  const url = process.env.CHAIN_URL;
  const { batchNumber } = req.body;
  
  if (!batchNumber) {
    return res.status(400).send({ error: 'batchNumber is required' });
  }

  try {
    // Call the JSON-RPC method
    const response = await axios.post(url!, {
      jsonrpc: "2.0",
      id: 1,
      method: "zks_getL1BatchBlockRange",
      params: [batchNumber]
    }, {
      headers: {
        'Content-Type': 'application/json'
      }
    });

    // Return the result
    res.send(response.data);
  } catch (error) {
    console.error(error);
    res.status(500).send({ error: 'Failed to fetch blocks' });
  }
});

// POST get Batch by number from txhash
router.post('/getBatchByTxHash', async (req, res) => {
  const url = process.env.CHAIN_URL;
  const { txHash } = req.body;
  
  if (!txHash) {
    return res.status(400).send({ error: 'txHash is required' });
  }

  try {
    // Call the JSON-RPC eth_getTransactionByHash to get block number
    const txDetail = await axios.post(url!, {
      jsonrpc: "2.0",
      id: 1,
      method: "eth_getTransactionByHash",
      params: [txHash]
    }, {
      headers: {
        'Content-Type': 'application/json'
      }
    });
    // Get the block number from the response
    const blockNumber = txDetail.data.result.blockNumber;
    // check if blockNumber is null or undefined
    if (!blockNumber) {
      return res.status(400).send({ error: 'Failed to fetch block number' });
    }
    // convert hex to decimal
    const blockNumberDec = parseInt(blockNumber, 16);
    console.log('blockNumber', blockNumberDec);
    // Call the JSON-RPC method
    const response = await axios.post(url!, {
      jsonrpc: "2.0",
      id: 1,
      method: "zks_getBlockDetails",
      params: [blockNumberDec]
    }, {
      headers: {
        'Content-Type': 'application/json'
      }
    });

    // Return the result
    res.send(response.data);
  } catch (error) {
    console.error(error);
    res.status(500).send({ error: 'Failed to fetch batch from tx hash' });
  }
});
export default router;