kd.exe -k com:pipe,port=\\.\pipe\debug,resets=0,reconnect -c "ed nt!Kd_Default_Mask 0xFFFFFFFF; g"
