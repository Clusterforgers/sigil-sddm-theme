#version 440

// A triangle that covers the whole item. Everything happens in the fragment stage, which
// works from gl_FragCoord, so there is nothing to pass along.
void main()
{
    float x = float(gl_VertexIndex / 2) * 4.0 - 1.0;
    float y = float(gl_VertexIndex & 1) * 4.0 - 1.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
}
