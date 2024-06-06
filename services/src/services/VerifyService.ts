import { exec } from 'child_process';
import { time } from 'console';

export const startVerifyService = (cmd: string) => {
  console.log(`[${new Date().toTimeString()}] Start verifying service ...`);

  // Example command to run (replace with your actual command)
  const command = cmd;
  let isRunning = false;

  // Function to execute the command
  const runCommand = () => {
    if (isRunning) {
      console.log("Previous command is still running, skipping this execution.");
      return;
    }
    
    isRunning = true;

    exec(command, (error, stdout, stderr) => {
      isRunning = false;
      if (error) {
        console.error(`Error executing command: ${error.message}`);
        return;
      }
      console.log(`Command stdout: ${stdout}`);
    });
  };

  // Run the command immediately
  runCommand();

  // Schedule the command to run every 10 minutes
  setInterval(runCommand, 60000);
};