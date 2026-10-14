storage:
  dbPath: "<<DATADIR>>"
systemLog:
  destination: file
  path: "<<LOG_DIR>>/mongod.log"
  logAppend: true
net:
  bindIp: 127.0.0.1
  port: <<PORT>>
security:
  authorization: disabled
