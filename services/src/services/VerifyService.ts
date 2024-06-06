import { exec } from 'child_process';

export const startVerifyService = (cmd: string) => {
  console.log('Starting verify Supersonic service...');

  // Example command to run (replace with your actual command)
  const command = cmd;

  // Function to execute the command
  const runCommand = () => {
    exec(command, (error, stdout, stderr) => {
      if (error) {
        console.error(`Error executing command: ${error.message}`);
        return;
      }
      if (stderr) {
        console.error(`Command stderr: ${stderr}`);
        return;
      }
      console.log(`Command stdout: ${stdout}`);
    });
  };

  // Run the command immediately
  runCommand();
  console.log("[Verify Service] Command executed successfully")

  // Schedule the command to run every 10 minutes
  setInterval(runCommand, 600000);
};