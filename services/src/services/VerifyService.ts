import { exec as _exec, spawn as _spawn } from 'child_process';


// executes a command in a new shell
// but pipes data to parent's stdout/stderr
export async function spawn(command: string) {
  command = command.replace(/\n/g, ' ');
  const child = _spawn(command, { stdio: 'inherit', shell: true });

  return new Promise((resolve, reject) => {
      child.on('error', (error) => {
          console.error('Error:', error);
          reject(error);
      });

      child.on('close', (code) => {
          if (code === 0) {
              resolve(code);
          } else {
              const errorMessage = `Child process exited with code ${code}`;
              console.error(errorMessage);
              reject(new Error(errorMessage));
          }
      });
  });
}


export async function startVerifyService() {
  console.log(`[${new Date().toTimeString()}] Start verifying service ...`);
  const cmd = 'cd ../ && ./target/release/state-reconstruct reconstruct l1 --http-url https://rpc-amoy.polygon.technology/ --da-url https://rpc-amoy.polygon.technology/'
  let isRunning = false;

  // Function to execute the command
  const runCommand = async () => {
    if (isRunning) {
      console.log("Previous command is still running, skipping this execution.");
      return;
    }
    await spawn(cmd)
    isRunning = false;
  };

  // Run the command immediately
  runCommand();

  // Schedule the command to run every 5 minutes
  setInterval(runCommand, 500000);
}