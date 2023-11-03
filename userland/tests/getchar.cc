#include <iostream>
#include <string>

int main() {
  std::string line;
  char c;

  while ((c = getchar()) != EOF) {
    std::cout << "Got: " << c << std::endl;
    // FIXME: Should we also break on \r?
    if (c == '\n')
      break;
    line.push_back(c);
  }

  std::cout << line << std::endl;
  return 0;
}
