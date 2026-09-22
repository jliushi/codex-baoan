//go:build ignore
package main
import ("bytes";"encoding/binary";"image";"image/color";"image/png";"math";"os")
func main(){
 const S=256; img:=image.NewRGBA(image.Rect(0,0,S,S))
 for y:=0;y<S;y++{for x:=0;x<S;x++{
  // rounded square mask
  r:=48.0; dx:=math.Max(0,math.Abs(float64(x)-S/2+0.5)-(S/2-r)); dy:=math.Max(0,math.Abs(float64(y)-S/2+0.5)-(S/2-r))
  if math.Hypot(dx,dy)>r{img.Set(x,y,color.RGBA{0,0,0,0});continue}
  // blue->teal diagonal gradient
  t:=float64(x+y)/(2*S)
  cr:=uint8(0x2f+ (0x2b-0x2f)*0); _=cr
  R:=uint8(0x2f+t*(0x2b-0x2f)); G:=uint8(0x7c+t*(0xb9-0x7c)); B:=uint8(0xf6+t*(0x9a-0xf6))
  img.Set(x,y,color.RGBA{R,G,B,0xff})
 }}
 // white shield-ish emblem: a centered rounded check/box
 for y:=0;y<S;y++{for x:=0;x<S;x++{
  cx,cy:=float64(x)-S/2,float64(y)-S/2
  if math.Abs(cx)<58&&math.Abs(cy)<66{
   // shield outline band
   if math.Abs(cx)>44||math.Abs(cy)>52{img.Set(x,y,color.RGBA{255,255,255,235})}
  }
 }}
 var pngbuf bytes.Buffer; png.Encode(&pngbuf,img); p:=pngbuf.Bytes()
 // ICO with a single PNG entry
 var b bytes.Buffer
 binary.Write(&b,binary.LittleEndian,uint16(0)) // reserved
 binary.Write(&b,binary.LittleEndian,uint16(1)) // type icon
 binary.Write(&b,binary.LittleEndian,uint16(1)) // count
 b.WriteByte(0);b.WriteByte(0) // 256x256 -> 0
 b.WriteByte(0);b.WriteByte(0) // colors, reserved
 binary.Write(&b,binary.LittleEndian,uint16(1)) // planes
 binary.Write(&b,binary.LittleEndian,uint16(32)) // bpp
 binary.Write(&b,binary.LittleEndian,uint32(len(p))) // size
 binary.Write(&b,binary.LittleEndian,uint32(22)) // offset
 b.Write(p)
 os.WriteFile("icon.ico",b.Bytes(),0o644)
 println("icon.ico",b.Len(),"bytes")
}
