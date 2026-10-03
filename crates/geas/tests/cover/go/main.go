// Areas of a few shapes, from the command line.
package main

import (
	"fmt"
	"os"
	"strconv"
)

func area(kind string, size int) int {
	switch kind {
	case "square":
		return size * size
	case "circle":
		return 3 * size * size
	}
	panic(kind)
}

func main() {
	size, _ := strconv.Atoi(os.Args[2])
	fmt.Println(area(os.Args[1], size))
}
