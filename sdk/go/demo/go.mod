module com.chanjet/go-sdk-demo

go 1.26.1

replace com.chanjet/connector-sdk-go => ..

require (
	com.chanjet/connector-sdk-go v0.0.0-00010101000000-000000000000
	github.com/joho/godotenv v1.5.1
)

require github.com/gorilla/websocket v1.5.3 // indirect
