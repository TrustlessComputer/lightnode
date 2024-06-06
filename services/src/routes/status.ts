import { Router, Request, Response } from 'express';

const router = Router();

interface Status {
    base_batch_number: number;
    bitcoin_tx_hash: string;
    da_tx_hash: string;
    batch_data: string;
}

interface StatusCache {
    lastest_batch_number: number;
    statuses: Status[];
}

// create a empty cache for status
const statusCache: StatusCache = {
    lastest_batch_number: 0,
    statuses: []
}

async function UpdateStatus() {
    const fs = require('fs');
    const path = require('path');
    const dir = path.join(__dirname, '../../../db-status');
    const files = fs.readdirSync(dir);
    // sort files name convert to number
    files.sort((a: string, b: string) => {
        return parseInt(a) - parseInt(b);
    });

    console.log('Reading status files...total files:', files.length);
    if (files.length === 0) {
        console.log('No files to read');
        return;
    } 
    if (files.length > statusCache.lastest_batch_number) {
        console.log('New files found, updating cache...');
        for (let i = statusCache.lastest_batch_number; i < files.length; i++) {
            //open file and read data
            const data = fs.readFileSync(path.join(dir, files[i]), 'utf8');
            // map data to object Status
            let status: Status = JSON.parse(data);
            status.batch_data = "";
            statusCache.statuses.push(status);
            statusCache.lastest_batch_number += 1;
        }
    }
    console.log('Status cache updated');
    console.log('Status latest_batch_number', statusCache.lastest_batch_number);
    console.log('DA TX of the lastest batch', statusCache.statuses[statusCache.statuses.length-1].da_tx_hash);
}

// Define the /status route
router.get('/status/:page/:per_page', (req: Request, res: Response) => {
    UpdateStatus();
    // pagination request based on query params page and per_page
    // clone the status and sort by base_batch_number desc
    const statuses = statusCache.statuses.slice().sort((a, b) => b.base_batch_number - a.base_batch_number);
    const page = Math.max(1, parseInt(req.query.page as string) || 1);
    const per_page = Math.max(10, parseInt(req.query.per_page as string) || 10);
    
    const total = statuses.length;
    const total_pages = Math.ceil(total/per_page);
    const start = (page-1)*per_page;
    const end = page*per_page;
    const data = statuses.slice(start, end);
    res.json({ page, per_page, total, total_pages, data });
});

export default router;