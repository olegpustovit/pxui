// CoreText reference raster for pxui_text's glyph_ref test (macOS only, built on demand):
//   clang -O1 -fobjc-arc -framework CoreText -framework CoreGraphics -framework Foundation coretext.m -o coretext
//   coretext <font.ttf> <size_pt> <scale> <black|white> <text> <out.pgm>
// Canvas 640x56 pt, baseline 40 pt from the top, origin x 8 pt; default context (smoothing on), sRGB
// RGBA, black-on-white or white-on-black. Writes the green channel as a binary PGM.
#import <CoreText/CoreText.h>
#import <Foundation/Foundation.h>
int main(int argc, char **argv) {
    if (argc != 7) return 2;
    CGFloat size = atof(argv[2]), scale = atof(argv[3]);
    BOOL white = argv[4][0] == 'w';
    NSURL *url = [NSURL fileURLWithPath:@(argv[1])];
    NSArray *descs = CFBridgingRelease(CTFontManagerCreateFontDescriptorsFromURL((__bridge CFURLRef)url));
    // pxui_text asks variable fonts for wght 400 (the Style default); pin CoreText to the same instance
    CTFontDescriptorRef d = CTFontDescriptorCreateCopyWithVariation((__bridge CTFontDescriptorRef)descs[0], (__bridge CFNumberRef)@(0x77676874), 400);
    CTFontRef font = CTFontCreateWithFontDescriptor(d, size, NULL);
    int w = 640 * scale, h = 56 * scale;
    CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef cx = CGBitmapContextCreate(NULL, w, h, 8, 0, cs, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
    CGContextSetRGBFillColor(cx, white ? 0 : 1, white ? 0 : 1, white ? 0 : 1, 1);
    CGContextFillRect(cx, CGRectMake(0, 0, w, h));
    CGContextScaleCTM(cx, scale, scale);
    CGContextSetRGBFillColor(cx, white ? 1 : 0, white ? 1 : 0, white ? 1 : 0, 1);
    NSDictionary *attrs = @{(__bridge id)kCTFontAttributeName: (__bridge id)font,
                            (__bridge id)kCTForegroundColorFromContextAttributeName: @YES};
    CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)[[NSAttributedString alloc] initWithString:@(argv[5]) attributes:attrs]);
    CGContextSetTextPosition(cx, 8, 16);
    CTLineDraw(line, cx);
    unsigned char *p = CGBitmapContextGetData(cx);
    size_t bpr = CGBitmapContextGetBytesPerRow(cx);
    FILE *f = fopen(argv[6], "wb");
    fprintf(f, "P5\n%d %d\n255\n", w, h);
    for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) fputc(p[y * bpr + x * 4 + 1], f);
    fclose(f);
    return 0;
}
