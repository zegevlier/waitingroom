pm2 stop node8008
pm2 stop node8007
pm2 stop node8006
pm2 stop node8005
pm2 stop node8004
pm2 stop node8003
pm2 stop node8002
pm2 stop node8001

sleep 2
pm2 restart int
pm2 restart demo

pm2 start node8001
sleep 0.5
pm2 start node8002
sleep 0.5
pm2 start node8003
sleep 0.5
pm2 start node8004
sleep 0.5
pm2 start node8005
sleep 0.5
pm2 start node8006
sleep 0.5
pm2 start node8007
sleep 0.5
pm2 start node8008