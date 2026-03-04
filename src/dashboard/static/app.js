// Fetch graph data and render force-directed layout
async function init() {
    const graphData = await fetch('/api/graph').then(r => r.json());
    const stats = await fetch('/api/stats').then(r => r.json());

    document.getElementById('stats').textContent =
        stats.files + ' files \u00b7 ' + stats.symbols + ' symbols \u00b7 ' +
        stats.calls + ' calls (' + (stats.resolution_rate * 100).toFixed(0) + '% resolved)';

    var width = window.innerWidth;
    var height = window.innerHeight - 60;

    var svg = d3.select('#graph')
        .attr('width', width)
        .attr('height', height);

    if (!graphData.nodes || graphData.nodes.length === 0) {
        svg.append('text')
            .attr('x', width / 2)
            .attr('y', height / 2)
            .attr('text-anchor', 'middle')
            .attr('fill', '#8b949e')
            .text('No graph data. Run ariadne index to populate.');
        return;
    }

    var simulation = d3.forceSimulation(graphData.nodes)
        .force('link', d3.forceLink(graphData.edges).id(function(d) { return d.id; }))
        .force('charge', d3.forceManyBody().strength(-100))
        .force('center', d3.forceCenter(width / 2, height / 2));

    var link = svg.selectAll('line')
        .data(graphData.edges)
        .join('line')
        .attr('stroke', '#999')
        .attr('stroke-opacity', 0.6);

    var node = svg.selectAll('circle')
        .data(graphData.nodes)
        .join('circle')
        .attr('r', 5)
        .attr('fill', function(d) { return d3.schemeCategory10[d.group % 10]; });

    node.append('title').text(function(d) { return d.name; });

    simulation.on('tick', function() {
        link
            .attr('x1', function(d) { return d.source.x; })
            .attr('y1', function(d) { return d.source.y; })
            .attr('x2', function(d) { return d.target.x; })
            .attr('y2', function(d) { return d.target.y; });
        node
            .attr('cx', function(d) { return d.x; })
            .attr('cy', function(d) { return d.y; });
    });
}

init();
