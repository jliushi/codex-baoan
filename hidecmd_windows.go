//go:build windows

package main

import (
	"os/exec"
	"syscall"
)

// hiddenCommand builds a command that never flashes a console window.
// CREATE_NO_WINDOW (0x08000000) keeps child processes (certutil) invisible.
func hiddenCommand(name string, args ...string) *exec.Cmd {
	cmd := exec.Command(name, args...)
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: 0x08000000}
	return cmd
}
