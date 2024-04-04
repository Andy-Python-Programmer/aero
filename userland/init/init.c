#include <fcntl.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

int main() {
  int fd_stdin = open("/dev/vtty", O_RDONLY);
  int fd_stdout = open("/dev/vtty", O_WRONLY);
  int fd_stderr = open("/dev/vtty", O_WRONLY);

  printf("Hello world\n");

  setenv("TERM", "linux", 1);
  setenv("USER", "root", 1);
  setenv("PATH", "/usr/local/bin:/usr/bin", 1);
  setenv("HOME", "/home/aero", 1);

  int pid = fork();

  if (!pid) {
    char *args[] = {"/usr/bin/bash", "--login", NULL};
    chdir(getenv("HOME"));
    execvp("/usr/bin/bash", args);
  } else {
    int status;
    waitpid(pid, &status, 0);
  }

  return 0;
}
