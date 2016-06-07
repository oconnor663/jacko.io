package main

import (
	"fmt"
)

type Foo struct {
	A int
	B int
}

func fooWriter(fooPtr *Foo) {
	i := 0
	for {
		newFoo := Foo{A: i, B: i}
		// When we create newFoo, A is always equal to B. However, struct
		// writes are NOT ATOMIC. That means that another tread reading fooPtr
		// might see a value of A that's not equal to B.
		*fooPtr = newFoo
		i++
	}
}

func main() {
	var myFoo Foo
	// Start a new thread that continuously writes to myFoo.
	go fooWriter(&myFoo)
	// Read the value of myFoo over and over until we see an inconsistent read.
	for {
		fooCopy := myFoo
		if fooCopy.A != fooCopy.B {
			fmt.Println("We got an inconsistent Foo!", fooCopy)
			return
		}
	}
}
