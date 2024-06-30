#include <assert.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

#define INITIAL_MSG "Hello, world!"
#define MSG_LEN (sizeof(INITIAL_MSG) - 1)

#define NEXT_MSG "Bye, world!"

int main() {
  int fd = open("/tmp/shared_file", O_CREAT | O_RDWR, 0644);
  write(fd, INITIAL_MSG, MSG_LEN);

  char *p = mmap(NULL, MSG_LEN, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
  pid_t pid = fork();

  if (!pid) {
    strncpy(p, NEXT_MSG, MSG_LEN);
    return EXIT_SUCCESS;
  }

  int wstatus;
  waitpid(pid, &wstatus, 0);
  assert(WIFEXITED(wstatus) && WEXITSTATUS(wstatus) == EXIT_SUCCESS);

  // ensure changes presist across processes
  assert(!strncmp(p, NEXT_MSG, MSG_LEN));
  munmap(p, MSG_LEN);

  // synchronize mapped region with its underlying file
  // msync(p, MSG_LEN, MS_SYNC);

  // ensure changes presist in the file
  char buf[MSG_LEN];
  lseek(fd, 0, SEEK_SET);
  read(fd, buf, MSG_LEN);
  assert(!strncmp(buf, NEXT_MSG, MSG_LEN));

  // cleanup
  close(fd);
  unlink("/tmp/shared_file");
  return EXIT_SUCCESS;
}
