#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>

#include <linux/prctl.h>  /* Definition of PR_* constants */
#include <sys/prctl.h>

int main() {
    pid_t pid;
    pid_t ppid;

    pid = getpid();
    ppid = getppid();

    if (prctl(PR_SET_CHILD_SUBREAPER, 1) < 0) {
        perror("prctl");
        exit(EXIT_FAILURE);
    }

    printf("TARGET PID: %d\n", pid);
    printf("TARGET PARENT PID: %d\n", ppid);

    exit(EXIT_SUCCESS);
}
