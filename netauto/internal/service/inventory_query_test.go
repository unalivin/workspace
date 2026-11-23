package service

import "testing"

func TestInventorySearchFiltersAndPages(t *testing.T) {
	s := testService(t)
	_, _ = s.AddDevice("edge-a", "mock", "host-a")
	_, _ = s.AddDevice("edge-b", "lab", "host-b")
	enabled := true
	got, e := s.SearchInventory(InventoryFilter{Adapter: "mock", Enabled: &enabled, Search: "edge", Limit: 10})
	if e != nil || len(got) != 1 || got[0].Name != "edge-a" {
		t.Fatalf("got %#v, %v", got, e)
	}
}
func TestInventoryRejectsBadPagination(t *testing.T) {
	if _, e := testService(t).SearchInventory(InventoryFilter{Limit: 501}); e == nil {
		t.Fatal("expected bounds error")
	}
}
func TestInventoryRejectsBadAdapter(t *testing.T) {
	if _, e := testService(t).SearchInventory(InventoryFilter{Adapter: "telnet"}); e == nil {
		t.Fatal("expected adapter error")
	}
}
func TestDeviceCount(t *testing.T) {
	s := testService(t)
	_, _ = s.AddDevice("r", "mock", "h")
	n, e := s.DeviceCount()
	if e != nil || n != 1 {
		t.Fatalf("got %d, %v", n, e)
	}
}
func TestDeviceDelete(t *testing.T) {
	s := testService(t)
	id, e := s.AddDevice("r", "mock", "h")
	if e != nil {
		t.Fatal(e)
	}
	if e = s.DeleteDevice(id); e != nil {
		t.Fatal(e)
	}
	if _, e = s.DeleteDevice(id); e == nil {
		t.Fatal("missing delete error")
	}
}
