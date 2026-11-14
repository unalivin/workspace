package words

import "testing"

func TestCountChoosesTheSingularOnlyForOne(t *testing.T) {
	for count, want := range map[int]string{0: "0 alerts", 1: "1 alert", 2: "2 alerts", -1: "-1 alerts"} {
		if got := Count(count, "alert", "alerts"); got != want {
			t.Errorf("Count(%d) = %q, want %q", count, got, want)
		}
	}
}
