pm2 delete all

sleep 2

pm2 start --name int "RUST_LOG="debug" cargo r --release -p waitingroom-http -- interface" --log interface.log
pm2 start --name demo "RUST_LOG="debug" cargo r --release -p waitingroom-http -- demo" --log demoserver.log

sleep 2

for i in $(seq 1 $1)
do
    pm2 start --name node$i "RUST_LOG="debug" cargo r --release -p waitingroom-http -- $(($i+8000)) 8001" --log node$i.log
    sleep 0.5
done