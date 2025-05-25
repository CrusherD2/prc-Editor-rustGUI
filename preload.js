const { contextBridge, ipcRenderer } = require('electron');
const fs = require('fs');
const path = require('path');

// Function to search for a file in multiple locations
async function findFile(filename) {
  const possibleLocations = [
    filename,
    path.join(__dirname, filename),
    path.join(__dirname, '..', filename),
    path.join(__dirname, '..', '..', filename),
    path.join(__dirname, 'assets', filename),
    path.join(__dirname, 'public', filename),
    path.join(process.cwd(), filename)
  ];
  
  for (const location of possibleLocations) {
    try {
      // Check if this location exists synchronously to avoid IPC overhead
      if (fs.existsSync(location)) {
        console.log(`Found file at ${location}`);
        return location;
      }
    } catch (e) {
      // Continue trying other locations
    }
  }
  
  // If not found, return the original path
  return filename;
}

// Expose protected methods that allow the renderer process to use
// the ipcRenderer without exposing the entire object
contextBridge.exposeInMainWorld(
  'api', {
    openFile: () => ipcRenderer.invoke('open-file-dialog'),
    saveFile: () => ipcRenderer.invoke('save-file-dialog'),
    readFile: async (filePath) => {
      try {
        // For ParamLabels.csv, try more locations before failing
        if (filePath === 'ParamLabels.csv' || filePath.endsWith('ParamLabels.csv')) {
          const foundPath = await findFile('ParamLabels.csv');
          return ipcRenderer.invoke('read-file', foundPath);
        }
        
        // Fall back to standard file read
        return ipcRenderer.invoke('read-file', filePath);
      } catch (error) {
        console.error(`Failed to read file: ${filePath}`, error);
        throw error;
      }
    },
    writeFile: (filePath, data) => ipcRenderer.invoke('write-file', filePath, data)
  }
); 