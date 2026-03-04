package main

import "fmt"

func greet(name string) string {
	return fmt.Sprintf("Hello, %s", name)
}

func main() {
	result := greet("World")
	fmt.Println(result)
}

type UserService struct {
	Name string
}

func (s *UserService) GetUser(id int) string {
	return s.Name
}
