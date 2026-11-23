package service

import (
 "fmt"
 "strings"
)

type InventoryFilter struct { Adapter string; Enabled *bool; Tag string; Search string; Limit int; Offset int }
// SearchInventory applies exact adapter and enabled filters, with escaped literal text matching.
func(s *Service) SearchInventory(f InventoryFilter)([]Device,error){
 if f.Limit==0{f.Limit=100};if f.Limit<1||f.Limit>500||f.Offset<0{return nil,fmt.Errorf("invalid pagination bounds")}
 if f.Adapter!=""&&f.Adapter!="mock"&&f.Adapter!="lab"&&f.Adapter!="ssh"{return nil,fmt.Errorf("unsupported adapter %q",f.Adapter)}
 enabled:=-1;if f.Enabled!=nil{enabled=0;if *f.Enabled{enabled=1}}
 term:="%"+strings.NewReplacer("%","\\%","_","\\_").Replace(strings.TrimSpace(f.Search))+"%"
 rows,err:=s.db.Conn.Query(`SELECT DISTINCT d.id,d.name,d.adapter,d.host,d.enabled FROM devices d LEFT JOIN device_tags t ON t.device_id=d.id WHERE (?='' OR d.adapter=?) AND (?=-1 OR d.enabled=?) AND (?='' OR t.tag=?) AND (?='%%' OR d.name LIKE ? ESCAPE '\' OR d.host LIKE ? ESCAPE '\') ORDER BY d.name LIMIT ? OFFSET ?`,f.Adapter,f.Adapter,enabled,enabled,f.Tag,f.Tag,term,term,term,f.Limit,f.Offset)
 if err!=nil{return nil,err};defer rows.Close();out:=[]Device{};for rows.Next(){var d Device;var active int;if err:=rows.Scan(&d.ID,&d.Name,&d.Adapter,&d.Host,&active);err!=nil{return nil,err};d.Enabled=active!=0;out=append(out,d)};return out,rows.Err()
}
func(s *Service) DeviceCount()(int,error){var count int;err:=s.db.Conn.QueryRow("SELECT COUNT(*) FROM devices").Scan(&count);return count,err}
func(s *Service) DeleteDevice(id int64)error{if id<=0{return fmt.Errorf("device id must be positive")};r,err:=s.db.Conn.Exec("DELETE FROM devices WHERE id=?",id);if err!=nil{return err};n,err:=r.RowsAffected();if err!=nil{return err};if n==0{return fmt.Errorf("device %d not found",id)};return nil}
