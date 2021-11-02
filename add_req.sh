i="0"

while [ $i -lt 100 ]
do
curl --location --request POST 'localhost:40146/api/chat/mythread' \
--header 'session_token: yzxwAD0WIO7bUtRrhmAaUPTvVZ06ct##1616234244' \
--header 'Content-Type: application/json' \
--data-raw '{
    "content": "Hello"
}'
i=$[$i+1]
done