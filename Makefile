pwd = $(shell pwd)
container_prefix = bldr
NO_CACHE = false
container_with = docker ps -a -q -f

.PHONY: container test run shell clean bldr-base package-clean packages

all: volumes container packages

package-clean:
	docker-compose run package bash -c 'rm -rf /opt/bldr/cache/pkgs/*'
	docker-compose run package bash -c 'rm -rf /opt/bldr/pkgs/*'

packages: package-clean
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package bash -c 'cd /src/packages; make world'

volume-clean: pkg-cache-volume-clean key-cache-volume-clean cargo-volume-clean installed-cache-volume-clean src-cache-volume-clean

volumes: pkg-cache-volume key-cache-volume cargo-volume installed-cache-volume src-cache-volume

installed-cache-volume:
	docker create -v /opt/bldr/pkgs --name bldr-installed-cache tianon/true /bin/true

installed-cache-volume-clean:
	if [ -n "`$(container_with) name=bldr-installed-cache`" ]; then docker rm bldr-installed-cache; fi

src-cache-volume:
	docker create -v /opt/bldr/cache/src --name bldr-src-cache tianon/true /bin/true

src-cache-volume-clean:
	if [ -n "`$(container_with) name=bldr-src-cache`" ]; then docker rm bldr-src-cache; fi

pkg-cache-volume:
	docker create -v /opt/bldr/cache/pkgs --name bldr-pkg-cache tianon/true /bin/true

pkg-cache-volume-clean:
	if [ -n "`$(container_with) name=bldr-pkg-cache`" ]; then docker rm bldr-pkg-cache; fi

key-cache-volume:
	docker create -v /opt/bldr/cache/keys --name bldr-keys-cache tianon/true /bin/true

key-cache-volume-clean:
	if [ -n "`$(container_with) name=bldr-keys-cache`" ]; then docker rm bldr-keys-cache; fi

cargo-volume:
	docker create -v /bldr-cargo-cache --name bldr-cargo-cache tianon/true /bin/true

cargo-volume-clean:
	if [ -n "`$(container_with) name=bldr-cargo-cache`" ]; then docker rm bldr-cargo-cache; fi

container:
	docker build -t chef/bldr --no-cache=${NO_CACHE} .

test:
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package cargo test

unit:
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package cargo test --lib

functional:
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package cargo test --test functional

cargo-clean:
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package cargo clean

docs:
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package cargo doc
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package rustdoc --crate-name bldr README.md -o ./target/doc/bldr
	docco -e .sh -o target/doc/bldr/bldr-build packages/bldr-build
	cp -r images ./target/doc/bldr
	echo '<meta http-equiv=refresh content=0;url=bldr/index.html>' > target/doc/index.html

doc-serve:
	@echo "View the docs at: http://127.0.0.1:9633/"
	ruby -run -e httpd -- --bind-address=127.0.0.1 --port=9633 ./target/doc

shell:
	docker-compose run bldr bash

pkg-shell:
	docker-compose run -e DOCKER_HOST=${DOCKER_HOST} package bash

bldr-base: packages

base-shell:
	docker-compose run base

clean:
	docker rm $(docker ps -q -f status=exited)
	docker images -q -f dangling=true | xargs docker rmi

redis:
	docker-compose run bldr cargo run -- start redis

publish:
	for x in `docker images | egrep '^bldr/base' | awk '{print $2}'`; do \
		docker tag -f bldr/base:$x quay.io/bldr/base:$x ; \
	done
	docker push quay.io/bldr/base
