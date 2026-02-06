const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("__ELECTRON__", {
  invoke: (cmd, payload) => ipcRenderer.invoke("anifrz:invoke", cmd, payload ?? {}),
  listen: (event, handler) => {
    const channel = `anifrz:event:${event}`;
    const wrapped = (_evt, payload) => {
      handler({ event, id: 0, payload });
    };
    ipcRenderer.on(channel, wrapped);
    return () => {
      ipcRenderer.removeListener(channel, wrapped);
    };
  },
});
