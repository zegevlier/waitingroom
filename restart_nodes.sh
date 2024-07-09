pm2 stop node8
pm2 stop node7
pm2 stop node6
pm2 stop node5
pm2 stop node4
pm2 stop node3
pm2 stop node2
pm2 stop node1

sleep 2
pm2 restart int
pm2 restart demo

pm2 start node1
sleep 0.5
pm2 start node2
sleep 0.5
pm2 start node3
sleep 0.5
pm2 start node4
sleep 0.5
pm2 start node5
sleep 0.5
pm2 start node6
sleep 0.5
pm2 start node7
sleep 0.5
pm2 start node8