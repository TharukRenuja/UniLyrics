package main

import (
	"fmt"
	"log"
	lyrics "github.com/TharukRenuja/UniLyrics/bindings/go"
)

func main() {
	parsed, err := lyrics.Parse("[00:01]Hello 世界 🎵", "lrc")
	if err != nil {
		log.Fatal(err)
	}
	converted, err := lyrics.Export(parsed.Document, "json", "strict")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(converted.Output)
}
