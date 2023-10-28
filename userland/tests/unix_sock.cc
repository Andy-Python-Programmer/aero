// test copied from managarm

#include <cassert>
#include <errno.h>
#include <iostream>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>
#include <vector>

#define NAMED_PATH "/tmp/sockname"

#define DEFINE_TEST(s, f) static test_case test_##s{#s, f};

struct abstract_test_case {
private:
  static void register_case(abstract_test_case *tcp);

public:
  abstract_test_case(const char *name) : name_{name} { register_case(this); }

  abstract_test_case(const abstract_test_case &) = delete;

  virtual ~abstract_test_case() = default;

  abstract_test_case &operator=(const abstract_test_case &) = delete;

  const char *name() { return name_; }

  virtual void run() = 0;

private:
  const char *name_;
};

template <typename F> struct test_case : abstract_test_case {
  test_case(const char *name, F functor)
      : abstract_test_case{name}, functor_{std::move(functor)} {}

  void run() override { functor_(); }

private:
  F functor_;
};

#define clean_errno() (errno == 0 ? "None" : strerror(errno))

#define log_error(M, ...)                                                      \
  fprintf(stderr, "[ERROR] %s:%d: errno: %s) " M "\n", __FILE__, __LINE__,     \
          clean_errno(), ##__VA_ARGS__)

#define assertf(A, M, ...)                                                     \
  if (!(A)) {                                                                  \
    log_error(M, ##__VA_ARGS__);                                               \
    assert(A);                                                                 \
  }

// clang-format off
DEFINE_TEST(unix_getname, ([] {
	int server_fd = socket(AF_UNIX, SOCK_STREAM, 0);
	if(server_fd == -1)
		assert(!"server socket() failed");

	struct sockaddr_un server_addr;
	memset(&server_addr, 0, sizeof(struct sockaddr_un));
	server_addr.sun_family = AF_UNIX;
	strncpy(server_addr.sun_path, NAMED_PATH, sizeof(server_addr.sun_path) - 1);

	if(bind(server_fd, (struct sockaddr *)&server_addr, sizeof(struct sockaddr_un)))
		assert(!"bind() failed");
	if(listen(server_fd, 50))
		assert(!"listen() failed");

	pid_t child = fork();
	if(!child) {
		int client_fd = socket(AF_UNIX, SOCK_STREAM, 0);
		if(client_fd == -1)
			assert(!"client socket() failed");
		if(connect(client_fd, (struct sockaddr *)&server_addr, sizeof(struct sockaddr_un)))
			assert(!"connect() to server failed");

		char buf[1];
		if (recv(client_fd, buf, 1, 0) < 0)
			assert(!"recv() failed");
		exit(0);
	} else {
		int peer_fd = accept(server_fd, nullptr, nullptr);
		if(peer_fd == -1)
			assert(!"accept() failed");

		struct sockaddr_un peer_addr;
		socklen_t peer_length = sizeof(struct sockaddr_un);
		if(getsockname(server_fd, (struct sockaddr *)&peer_addr, &peer_length))
			assert(!"getsockname(server) failed");
		assert(peer_length == (offsetof(sockaddr_un, sun_path) + 14));
		assert(!strcmp(peer_addr.sun_path, NAMED_PATH));

		memset(&peer_addr, 0, sizeof(struct sockaddr));
		peer_length = sizeof(struct sockaddr_un);
		if(getsockname(peer_fd, (struct sockaddr *)&peer_addr, &peer_length))
			assert(!"getsockname(peer) failed");
				printf("peer_len=%d\n", peer_length);
			printf("wanted=%d\n", offsetof(sockaddr_un, sun_path) + 14);
		assert(peer_length == (offsetof(sockaddr_un, sun_path) + 14));
		assert(!strcmp(peer_addr.sun_path, NAMED_PATH));

		memset(&peer_addr, 0, sizeof(struct sockaddr));
		peer_length = sizeof(struct sockaddr_un);
		if(getpeername(peer_fd, (struct sockaddr *)&peer_addr, &peer_length))
			assert(!"getpeername(peer) failed");
				printf("peer_len=%d\n", peer_length);
			printf("wanted=%d\n", offsetof(sockaddr_un, sun_path) );
		assert(peer_length == offsetof(sockaddr_un, sun_path));

		char buf[1]{0};
		if (send(peer_fd, buf, 1, 0) < 0)
			assert(!"send() failed");
	}
	unlink(NAMED_PATH);
}));

std::vector<abstract_test_case *> &test_case_ptrs() {
	static std::vector<abstract_test_case *> singleton;
	return singleton;
}

void abstract_test_case::register_case(abstract_test_case *tcp) {
	test_case_ptrs().push_back(tcp);
}

int main() {
    // Go through all tests and run them.
    for(abstract_test_case *tcp : test_case_ptrs()) {
		std::cout << "tests: Running " << tcp->name() << std::endl;
		tcp->run();
	}
}