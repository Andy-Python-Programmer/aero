// clang-format off
//
// xbstrap runtool host-gcc -- x86_64-aero-g++ ../userland/tests/test.cc -o system-root/torture 

#include <asm/unistd_64.h>
#include <cassert>
#include <fcntl.h>
#include <csetjmp>
#include <fstream>
#include <sys/stat.h>
#include <errno.h>
#include <iostream>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/socket.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>
#include <vector>
#include <cassert>

#if defined(__aero__)
#include <aero/syscall.h>
#elif defined(__linux__)
#include <sys/syscall.h>
#else
#error "unknown platform"
#endif

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


#define PAGE_SIZE 4096

void enable_systrace() {
#define SYS_TRACE 71
	long ret;
	asm volatile("syscall" : "=a"(ret) : "a"(SYS_TRACE) : "rcx", "r11", "memory");
}

#define assert_errno(fail_func, expr) ((void)(((expr) ? 1 : 0) || (assert_errno_fail(fail_func, #expr, __FILE__, __PRETTY_FUNCTION__, __LINE__), 0)))

inline void assert_errno_fail(const char *fail_func, const char *expr,
		const char *file, const char *func, int line) {
	int err = errno;
	fprintf(stderr, "In function %s, file %s:%d: Function %s failed with error '%s'; failing assertion: '%s'\n",
			func, file, line, fail_func, strerror(err), expr);
	abort();
	__builtin_unreachable();
}

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
		assert(peer_length == (offsetof(sockaddr_un, sun_path) + 14));
		assert(!strcmp(peer_addr.sun_path, NAMED_PATH));

		memset(&peer_addr, 0, sizeof(struct sockaddr));
		peer_length = sizeof(struct sockaddr_un);
		if(getpeername(peer_fd, (struct sockaddr *)&peer_addr, &peer_length))
			assert(!"getpeername(peer) failed");
		assert(peer_length == offsetof(sockaddr_un, sun_path));

		char buf[1]{0};
		if (send(peer_fd, buf, 1, 0) < 0)
			assert(!"send() failed");
	}
	unlink(NAMED_PATH);
}));

DEFINE_TEST(epoll_mod_active, ([] {
	int e;
	int pending;

	int fd = eventfd(0, 0);
	assert(fd >= 0);

	int epfd = epoll_create1(0);
	assert(epfd >= 0);

	epoll_event evt;

	memset(&evt, 0, sizeof(epoll_event));
	evt.events = 0;
	e = epoll_ctl(epfd, EPOLL_CTL_ADD, fd, &evt);
	assert(!e);

	// Nothing should be pending.
	memset(&evt, 0, sizeof(epoll_event));
	pending = epoll_wait(epfd, &evt, 1, 0);
	assert(!pending);

	uint64_t n = 1;
	auto written = write(fd, &n, sizeof(uint64_t));
	assert(written == sizeof(uint64_t));

	memset(&evt, 0, sizeof(epoll_event));
	evt.events = EPOLLIN;
	e = epoll_ctl(epfd, EPOLL_CTL_MOD, fd, &evt);
	assert(!e);

	// The FD should be pending now.
	memset(&evt, 0, sizeof(epoll_event));
	pending = epoll_wait(epfd, &evt, 1, 0);
	assert(pending == 1);
	assert(evt.events & EPOLLIN);

	close(epfd);
	close(fd);
}))

// Use mmap to change the protection flags instead of mprotect.
DEFINE_TEST(mmap_partial_remap, ([] {
	//enable_systrace();

	const int bytes = PAGE_SIZE * 2;

	void *result = mmap(nullptr, bytes, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	assert(result != MAP_FAILED);

	void *x = mmap(result, PAGE_SIZE, PROT_NONE, MAP_FIXED | MAP_PRIVATE | MAP_ANON, -1, 0);
	assert(x != MAP_FAILED);

	void *y = mmap(static_cast<char *>(result) + bytes - PAGE_SIZE, PAGE_SIZE, PROT_NONE, MAP_FIXED | MAP_PRIVATE | MAP_ANON, -1, 0);
	assert(y != MAP_FAILED);
}))

namespace {
	void *offsetBy(void *ptr, ptrdiff_t n) {
		return reinterpret_cast<void *>(
				reinterpret_cast<uintptr_t>(ptr)
					+ n);
	}

	sigjmp_buf restoreEnv;

	void signalHandler(int, siginfo_t *, void *) {
		siglongjmp(restoreEnv, 1);
	}

	bool ensureReadable(void *ptr) {
		if (sigsetjmp(restoreEnv, 1)) {
			return false;
		}

		(void)(*reinterpret_cast<volatile uint8_t *>(ptr));

		return true;
	}

	bool ensureWritable(void *ptr) {
		if (sigsetjmp(restoreEnv, 1)) {
			return false;
		}

		*reinterpret_cast<volatile uint8_t *>(ptr) = 0;

		return true;
	}

	bool ensureNotReadable(void *ptr) {
		if (sigsetjmp(restoreEnv, 1)) {
			return true;
		}

		(void)(*reinterpret_cast<volatile uint8_t *>(ptr));

		return false;
	}

	bool ensureNotWritable(void *ptr) {
		if (sigsetjmp(restoreEnv, 1)) {
			return true;
		}

		*reinterpret_cast<volatile uint8_t *>(ptr) = 0;

		return false;
	}

	template <typename Func>
	void runChecks(Func &&f) {
		pid_t pid = fork();
		assert_errno("fork", pid >= 0);

		struct sigaction sa, old_sa;
		sigemptyset(&sa.sa_mask);
		sa.sa_sigaction = signalHandler;
		sa.sa_flags = SA_SIGINFO;

		int ret = sigaction(SIGSEGV, &sa, &old_sa);
		assert_errno("sigaction", ret != -1);

		if (pid == 0) {
			f();
			exit(0);
		} else {
			int status = 0;
			while (waitpid(pid, &status, 0) == -1) {
				if (errno == EINTR) continue;
				assert_errno("waitpid", false);
			}

			if (WIFSIGNALED(status) || WEXITSTATUS(status) != 0) {
				fprintf(stderr, "Test failed on subprocess!\n");
				abort();
			}

			f();
		}

		ret = sigaction(SIGSEGV, &old_sa, nullptr);
		assert_errno("sigaction", ret != -1);
	}

	const size_t pageSize = sysconf(_SC_PAGESIZE);
} // namespace anonymous

DEFINE_TEST(mmap_fixed_replace_middle, ([] {
	void *mem = mmap(nullptr, pageSize * 3, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	void *newPtr = mmap(offsetBy(mem, pageSize), pageSize, PROT_READ, MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED, -1, 0);
	assert_errno("mmap", newPtr != MAP_FAILED);
	assert(newPtr == offsetBy(mem, pageSize));

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));

		assert(ensureReadable(offsetBy(mem, pageSize * 2)));
		assert(ensureWritable(offsetBy(mem, pageSize * 2)));
	});

	int ret = munmap(mem, pageSize * 3);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));

		assert(ensureNotReadable(offsetBy(mem, pageSize * 2)));
		assert(ensureNotWritable(offsetBy(mem, pageSize * 2)));
	});
}))

DEFINE_TEST(mmap_fixed_replace_left, ([] {
	void *mem = mmap(nullptr, pageSize * 2, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	void *newPtr = mmap(mem, pageSize, PROT_READ, MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED, -1, 0);
	assert_errno("mmap", newPtr != MAP_FAILED);
	assert(newPtr == mem);

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureWritable(offsetBy(mem, pageSize)));
	});

	int ret = munmap(mem, pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});
}))

DEFINE_TEST(mmap_fixed_replace_right, ([] {
	void *mem = mmap(nullptr, pageSize * 2, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	void *newPtr = mmap(offsetBy(mem, pageSize), pageSize, PROT_READ, MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED, -1, 0);
	assert_errno("mmap", newPtr != MAP_FAILED);
	assert(newPtr == offsetBy(mem, pageSize));

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});

	int ret = munmap(mem, pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});
}))

DEFINE_TEST(mmap_partial_protect_middle, ([] {
	void *mem = mmap(nullptr, pageSize * 3, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = mprotect(offsetBy(mem, pageSize), pageSize, PROT_READ);
	assert_errno("mprotect", ret != -1);

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));

		assert(ensureReadable(offsetBy(mem, pageSize * 2)));
		assert(ensureWritable(offsetBy(mem, pageSize * 2)));
	});

	ret = munmap(mem, pageSize * 3);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));

		assert(ensureNotReadable(offsetBy(mem, pageSize * 2)));
		assert(ensureNotWritable(offsetBy(mem, pageSize * 2)));
	});
}))

DEFINE_TEST(mmap_partial_protect_left, ([] {
	void *mem = mmap(nullptr, pageSize * 2, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = mprotect(mem, pageSize, PROT_READ);
	assert_errno("mprotect", ret != -1);

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureWritable(offsetBy(mem, pageSize)));
	});

	ret = munmap(mem, pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});
}))

DEFINE_TEST(mmap_partial_protect_right, ([] {
	void *mem = mmap(nullptr, pageSize * 2, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = mprotect(offsetBy(mem, pageSize), pageSize, PROT_READ);
	assert_errno("mprotect", ret != -1);

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});

	ret = munmap(mem, pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});
}))

DEFINE_TEST(mmap_partial_unmap_middle, ([] {
	void *mem = mmap(nullptr, pageSize * 3, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = munmap(offsetBy(mem, pageSize), pageSize);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));

		assert(ensureReadable(offsetBy(mem, pageSize * 2)));
		assert(ensureWritable(offsetBy(mem, pageSize * 2)));
	});

	ret = munmap(mem, pageSize * 3);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));

		assert(ensureNotReadable(offsetBy(mem, pageSize * 2)));
		assert(ensureNotWritable(offsetBy(mem, pageSize * 2)));
	});
}))

DEFINE_TEST(mmap_partial_unmap_left, ([] {
	void *mem = mmap(nullptr, pageSize * 2, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = munmap(mem, pageSize);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureReadable(offsetBy(mem, pageSize)));
		assert(ensureWritable(offsetBy(mem, pageSize)));
	});

	ret = munmap(mem, pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});
}))

DEFINE_TEST(mmap_partial_unmap_right, ([] {
	void *mem = mmap(nullptr, pageSize * 2, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = munmap(offsetBy(mem, pageSize), pageSize);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureReadable(mem));
		assert(ensureWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});

	ret = munmap(mem, pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));

		assert(ensureNotReadable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize)));
	});
}))

DEFINE_TEST(mmap_unmap_range_before_first, ([] {
	void *mem = mmap(reinterpret_cast<void *>(0x100000 + pageSize * 2), pageSize,
			PROT_READ | PROT_WRITE, MAP_FIXED | MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);

	int ret = munmap(reinterpret_cast<void *>(0x100000 + pageSize), pageSize * 2);
	assert_errno("munmap", ret != -1);

	runChecks([&] {
		assert(ensureNotReadable(mem));
		assert(ensureNotWritable(mem));
	});
}))

DEFINE_TEST(mprotect_check_whether_split_mappings_get_protected_correctly, ([] {
	void *mem = mmap(NULL, 0x6000, PROT_READ | PROT_EXEC, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);
	int ret = mprotect(mem, 0x1000, PROT_READ | PROT_WRITE);
	assert_errno("mprotect", ret != -1);
	ret = mprotect(mem, 0x1000, PROT_READ | PROT_EXEC);
	assert_errno("mprotect", ret != -1);
	ret = mprotect(mem, 0x5000, PROT_READ | PROT_WRITE);
	assert_errno("mprotect", ret != -1);

	runChecks([&] {
		assert(ensureWritable(mem));
	});
}))

DEFINE_TEST(mprotect_check_whether_three_way_split_mappings_are_handled_correctly, ([] {
	void *mem = mmap(NULL, pageSize * 3, PROT_READ, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
	assert_errno("mmap", mem != MAP_FAILED);
	int ret = mprotect(offsetBy(mem, pageSize), pageSize, PROT_READ | PROT_WRITE);
	assert_errno("mprotect", ret != -1);

	runChecks([&] {
		assert(ensureNotWritable(mem));
		assert(ensureWritable(offsetBy(mem, pageSize)));
		assert(ensureNotWritable(offsetBy(mem, pageSize * 2)));
	});
}))

DEFINE_TEST(stat, ([] {
	// SYM_B -> SYM_A -> /tmp/SYM_REAL

	// TODO: make mknod()
	FILE *sym_real = fopen("/tmp/SYM_REAL", "w");

	if (symlink("/tmp/SYM_REAL", "/tmp/SYM_A") == -1) 
		assert(!"(1) symlink() failed");

	if (symlink("/tmp/SYM_A", "/tmp/SYM_B") == -1) 
		assert(!"(2) symlink() failed");

	struct stat statbuf;
	if (fstatat(AT_FDCWD, "/tmp/SYM_B", &statbuf, AT_SYMLINK_NOFOLLOW) == -1) 
		assert(!"fstatat() failed");

	// Check that the symlink is not followed.
	assert(S_ISLNK(statbuf.st_mode));

	if (fstatat(AT_FDCWD, "/tmp/SYM_B", &statbuf, 0) == -1) 
		assert(!"fstatat() failed");

	// Check that the symlink is followed.
	assert(S_ISREG(statbuf.st_mode));

	if (unlink("/tmp/SYM_A") == -1) 
		assert(!"unlink() failed");

	if (unlink("/tmp/SYM_B") == -1) 
		assert(!"unlink() failed");

	fclose(sym_real);
	if (unlink("/tmp/SYM_REAL") == -1)
		assert(!"unlink() failed");
}))

static inline bool cpuid(uint32_t leaf, uint32_t subleaf,
                         uint32_t *eax, uint32_t *ebx, uint32_t *ecx, uint32_t *edx)  {
	uint32_t cpuid_max;
    asm volatile ("cpuid"
                  : "=a" (cpuid_max)
                  : "a" (leaf & 0x80000000) : "rbx", "rcx", "rdx");
    if (leaf > cpuid_max)
        return false;
    asm volatile ("cpuid"
                  : "=a" (*eax), "=b" (*ebx), "=c" (*ecx), "=d" (*edx)
                  : "a" (leaf), "c" (subleaf));
    return true;
}

// Returns [`true`] if the `SYSENTER` and `SYSEXIT` and associated MSRs are supported.
bool has_sysenter_sysexit() {
	uint32_t eax, ebx, ecx, edx;
	// LEAF 1: Processor and Processor Feature Identifiers
	if (!cpuid(1, 0, &eax, &ebx, &ecx, &edx)) {
		return false;
	}
	return edx & (1 << 11);
}

#if defined(__aero__)
DEFINE_TEST(bad_sysenter, ([] {
	if (!has_sysenter_sysexit()) {
		printf("test skipped... sysenter not supported\n");
		return;
	}

	int pid = fork();

	if (!pid) {
		register long r11 __asm__("r11") = (size_t)0xf0f0 << 48;
		register long rcx __asm__("rcx") = (size_t)0xf0f0 << 48;

		asm volatile(
			"sysenter\n"
			:  	
			: "r"(r11), "r"(rcx)
		);
		
		__builtin_unreachable();
	} else {
		int status = 0;
		if (!waitpid(pid, &status, 0))
			assert(!"waitpid() failed");

		// FIXME: should we get killed with SIGSEGV instead?
		assert(WIFEXITED(status));
	}
}))
#endif

#if defined(__aero__)
DEFINE_TEST(sysenter_system_call, ([] {
	if (!has_sysenter_sysexit()) {
		printf("test skipped... sysenter not supported\n");
		return;
	}

	int fds[2];
	if (pipe(fds) == -1)
		assert(!"pipe() failed");

	int pid = fork();

	if (!pid) {
		close(fds[0]);

		const char *buf = "Hello, world!\n";
		size_t buf_size = strlen(buf);

		__asm__ __volatile__ (
			"mov %%rsp, %%r11\n\t"
			"lea 1f(%%rip), %%rcx\n\t"
			"sysenter\n\t"
			"1:"
			:
			: "a"(uint64_t(1)), "D"(uint64_t(fds[1])), "S"(buf), "d"(buf_size + 1)
			: "rcx", "r11"
		);

		exit(0);
	} else {
		close(fds[1]);

		int status = 0;
		if (!waitpid(pid, &status, 0))
			assert(!"waitpid() failed");

		assert(WIFEXITED(status));

		char tmp[15];
		ssize_t n = read(fds[0], tmp, sizeof(tmp));

		assert(n == 15);
		assert(!strcmp(tmp, "Hello, world!\n"));
	}
}))
#endif

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
