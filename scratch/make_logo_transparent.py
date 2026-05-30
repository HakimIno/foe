import os
from PIL import Image, ImageDraw

def make_logo_transparent(input_path, output_path):
    print(f"Processing {input_path}...")
    
    # Open image and ensure it's RGBA
    img = Image.open(input_path).convert("RGBA")
    width, height = img.size
    
    # Let's find the seed colors (corners)
    # The background is blue, so we'll floodfill from the four corners.
    # We use a threshold to handle the blue gradient.
    # A thresh of 100 in RGB distance should work well to capture the blue gradient
    # while stopping at the white letters and circle boundaries.
    
    # We will do flood fill from:
    # 1. Top-Left: (0, 0)
    # 2. Top-Right: (width - 1, 0)
    # 3. Bottom-Left: (0, height - 1)
    # 4. Bottom-Right: (width - 1, height - 1)
    
    # Transparent color: (0, 0, 0, 0)
    fill_color = (0, 0, 0, 0)
    
    # We will perform flood fill on the image directly using ImageDraw.floodfill
    # Pillow's floodfill modifies the image in place.
    # Note: floodfill in Pillow might need thresh to be specified.
    # If the default floodfill is not flexible enough, we can implement a custom BFS.
    
    # Let's implement a custom BFS floodfill with RGB distance threshold
    # to be 100% sure it works perfectly with the gradient.
    pixels = img.load()
    visited = set()
    queue = [(0, 0), (width - 1, 0), (0, height - 1), (width - 1, height - 1)]
    
    # Define a helper to check if a pixel is "blue background"
    # The letters and inner emblem are white, red, dark blue, or dark red.
    # The outer background is a bright/sky blue.
    # Let's check if a pixel's blue component is dominant and red/green are lower,
    # or simply if it's far from white/red/etc.
    # Actually, a simple threshold distance from the starting corner pixel color works perfectly!
    
    start_colors = [pixels[0, 0], pixels[width - 1, 0], pixels[0, height - 1], pixels[width - 1, height - 1]]
    
    def color_dist(c1, c2):
        # Calculate Euclidean distance in RGB space
        return ((c1[0] - c2[0])**2 + (c1[1] - c2[1])**2 + (c1[2] - c2[2])**2)**0.5

    # BFS
    for x, y in queue:
        visited.add((x, y))
        
    head = 0
    while head < len(queue):
        cx, cy = queue[head]
        head += 1
        
        curr_color = pixels[cx, cy]
        
        # Set to transparent
        pixels[cx, cy] = fill_color
        
        # Check neighbors
        for dx, dy in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
            nx, ny = cx + dx, cy + dy
            if 0 <= nx < width and 0 <= ny < height and (nx, ny) not in visited:
                neighbor_color = pixels[nx, ny]
                
                # Check distance to starting corner colors
                # If it matches any of the start colors (which are blue background) within threshold,
                # we continue flood-filling.
                # Threshold of 80 is good for gradients.
                is_bg = False
                for start_color in start_colors:
                    if color_dist(neighbor_color, start_color) < 85:
                        is_bg = True
                        break
                        
                # Ensure we don't accidentally fill white letters (which are close to (255,255,255))
                # or red stripes (high red, low blue).
                # Blue background typically has high blue and lower red.
                # White color distance to start_color (which is blue) is very high (usually > 150).
                if is_bg:
                    visited.add((nx, ny))
                    queue.append((nx, ny))
                    
    # Save the processed image
    img.save(output_path, "PNG")
    print(f"Saved transparent image to {output_path}")

if __name__ == "__main__":
    make_logo_transparent(
        "/Users/weerachit/Documents/foe/scratch/searxng-local/custom-ui/logo.png",
        "/Users/weerachit/Documents/foe/scratch/searxng-local/custom-ui/logo_transparent.png"
    )
    make_logo_transparent(
        "/Users/weerachit/Documents/foe/browser-ui/ui/logo.png",
        "/Users/weerachit/Documents/foe/browser-ui/ui/logo_transparent.png"
    )
